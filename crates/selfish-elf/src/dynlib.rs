//! Building the vendor dynamic segment - the writing side of [`crate::dynamic`].
//!
//! A linker produces an ordinary dynamic ELF. What a console loader wants is different in
//! three ways, and none of them can be expressed to the linker:
//!
//! 1. The tables live in a `PT_SCE_DYNLIBDATA` segment, addressed by **offsets into that
//!    segment** rather than by virtual address.
//! 2. Every imported symbol is named `<hash>#<library>#<module>` rather than by its plain
//!    name, and is typed as a function.
//! 3. The dynamic table carries vendor tags the linker knows nothing about, including three
//!    per imported library.
//!
//! So the tables are rebuilt from the linked ones and appended. This module does the
//! rebuilding; [`crate::dynamic`] reads the result back, and a round trip between them is the
//! test principle 4 asks for.
//!
//! # What is a caller's business
//!
//! Which library resolves a given name. That is a manifest, not a fact about the format, and
//! it arrives as a closure. Everything else here is the format.
//!
//! # Ordering is load-bearing, twice
//!
//! **The string table is declared first.** A loader walks the tags in order and resolves a
//! name offset the moment it meets one; four of the tags below carry a name offset. Emitted
//! before the string table is declared, they dereference a base the loader does not have yet -
//! a fault inside the loader, before a guest instruction runs, with nothing in its log.
//!
//! **The tables are laid out end to end.** A reference module places them adjacently, which is
//! how the tag meanings were established in the first place: `JMPREL + PLTRELSZ == RELA`, and
//! `RELA + RELASZ == HASH`. Emitting the same adjacency keeps that arithmetic true of output
//! as well as of input, so the same check that identified the tags also checks the writer.

use selfish_abi::Generation;
use selfish_nid::Nid;

use crate::dynamic::{Table, Tags, standard, vendor};
use crate::reloc::RELA_SIZE;
use crate::section::SYMBOL_SIZE;

/// `st_info` type for a function.
///
/// # Why every import is typed
///
/// A linker leaves an undefined reference as `STT_NOTYPE`: it knows the name is wanted and
/// nothing else, and on an ordinary system that is enough because the dynamic linker matches
/// on the name alone.
///
/// A console loader does not. It matches on the hash **and the symbol type**, against a table
/// where every platform function is registered as a function - so an import typed `NOTYPE`
/// matches nothing. It does not fail: it binds to a stub that returns zero. The module loads,
/// runs, and gets a plausible zero back from every call it makes.
pub const FUNCTION: u8 = 2;

/// `st_info` binding for an import the loader is expected to bind.
///
/// # Why every import is re-bound, and what weak costs
///
/// A probe declares its platform functions weak, so a symbol the platform does not have is
/// null rather than a link error. That is a **compile-time** need and it is a good one. What
/// it leaves behind is a dynamic symbol table where every import is `STB_WEAK`, undefined -
/// and to a loader that means precisely "if resolving this costs anything, do not bother".
///
/// A loader is entitled to take that at its word, and one does. Measured: a module whose 203
/// imports were all `WEAK FUNC` had them bound **only from the two libraries already resident
/// in the process** - `libkernel` and `libSceLibcInternal`, where resolution is free. Every
/// other declared library was mapped into the address space, with an address range and a
/// fingerprint in the system log, and not one of its symbols was bound. Fourteen imports
/// stayed null whose symbols the same process could find by name moments later.
///
/// A title that launches has no weak imports at all: 126 `GLOBAL FUNC` and 13 `GLOBAL OBJECT`.
///
/// So the binding is rewritten here for the same reason the type is (see [`FUNCTION`]): the
/// name is what a reader looks at and it is not what the loader decides on. The weak binding
/// stays where it is needed, in the C, and does not travel into the module. (obscene#D248)
pub const GLOBAL: u8 = 1;

/// Version numbers a module and its libraries declare.
pub mod version {
    /// The default module version, major. See [`super::module_version`].
    pub const MODULE_MAJOR: u8 = 1;
    /// The default module version, minor.
    pub const MODULE_MINOR: u8 = 1;
    /// The version every platform library is registered with.
    ///
    /// **Not cosmetic.** A loader builds its lookup key from the version *this module
    /// declares* and matches it against the version the library was registered with, so
    /// declaring zero against a library registered as one does not match - and every symbol
    /// from it silently fails to resolve. The module loads, runs, and finds none of its
    /// imports.
    pub const LIBRARY: u16 = 1;
    /// The library attribute meaning "export everything automatically".
    pub const AUTO_EXPORT: u64 = 0x1;
    /// The attribute word an **import** library carries, as a real launching title writes it.
    ///
    /// Measured, not reasoned: every one of the twenty-two `DT_SCE_IMPORT_LIB_ATTR` entries in
    /// a title that launches on retail hardware carries `0x9`. This crate wrote
    /// [`AUTO_EXPORT`] here instead, reusing the export attribute for the import side because
    /// the two words are the same shape and one constant covered both without complaint.
    ///
    /// **What it cost.** A module built that way loads, runs, and binds imports only from
    /// libraries the process already had - `libkernel` and `libSceLibcInternal`, which are
    /// resident before the module is looked at. Every other declared library was mapped into
    /// the address space, with an address range and a fingerprint in the system log, and not
    /// one of its symbols bound. Twenty-four checks skipped saying the loader had not resolved
    /// the symbol, which was true and read as a statement about the platform.
    ///
    /// **Bit 3 is deliberately not named.** Its meaning is not established - only that a real
    /// title sets it on every import library and that a module without it does not get its
    /// imports bound. Naming it `AUTO_LOAD` would be inventing a fact to make the constant
    /// read nicely, which is the failure D008 exists to prevent. It is called what it is: the
    /// attribute an import library carries.
    pub const IMPORT_LIBRARY: u64 = 0x9;
    /// What `DT_SCE_PLTREL` states: the linkage relocations are `Elf64_Rela`.
    pub const RELA_FORM: u64 = 7;
}

/// Bytes reserved at the head of the vendor segment for the module's fingerprint.
///
/// A real executable puts sixteen bytes of build identifier here and pads to `0x18`, then
/// starts its string table. `DT_SCE_FINGERPRINT` carries this region's offset, which is zero.
///
/// **Written as zeroes**, like every other digest and signature area in this project. A
/// fingerprint identifies a build; it authenticates nothing, and inventing a plausible-looking
/// one would be a value in a field nothing here can justify. See principles 5 and 6.
pub const FINGERPRINT_SIZE: u64 = 0x18;

/// Pack an id and a name offset, the way the identity tags do.
#[must_use]
pub const fn identity(id: u16, version: u16, name_offset: u32) -> u64 {
    ((id as u64) << 48) | ((version as u64) << 32) | name_offset as u64
}

/// Pack an id, a two-part version, and a name offset.
#[must_use]
pub const fn module_identity(id: u16, major: u8, minor: u8, name_offset: u32) -> u64 {
    ((id as u64) << 48) | ((major as u64) << 40) | ((minor as u64) << 32) | name_offset as u64
}

/// The table of library versions that are not the default.
const LIBRARY_VERSIONS: &str = include_str!("../../../data/library-versions.tsv");

/// The module version to declare for one library, at one generation.
///
/// [`version::MODULE_MAJOR`]`.`[`version::MODULE_MINOR`] unless `data/library-versions.tsv`
/// carries a row saying otherwise, and today it carries exactly one.
///
/// **This is not a constant, and the reason is worth reading before making it one again.** A
/// loader matches the version a module declares against the version the library was
/// registered with, and a mismatch resolves nothing from that library - silently. The one
/// library with a row here is the display library, so the symptom is a module that runs
/// perfectly and draws a black window.
#[must_use]
pub fn module_version(library: &str, generation: Generation) -> (u8, u8) {
    let wanted = match generation {
        Generation::Current => "5",
        Generation::Previous => "4",
    };
    for line in LIBRARY_VERSIONS.lines() {
        if line.starts_with('#') {
            continue;
        }
        let mut columns = line.split('\t');
        let (Some(name), Some(at), Some(major), Some(minor)) = (
            columns.next(),
            columns.next(),
            columns.next(),
            columns.next(),
        ) else {
            continue;
        };
        if name != library || at != wanted {
            continue;
        }
        if let (Ok(major), Ok(minor)) = (major.trim().parse(), minor.trim().parse()) {
            return (major, minor);
        }
    }
    (version::MODULE_MAJOR, version::MODULE_MINOR)
}

/// Pack an id and an attribute.
#[must_use]
pub const fn attribute(id: u16, attr: u64) -> u64 {
    ((id as u64) << 48) | attr
}

/// What a manifest says about one imported symbol.
///
/// The identifier is the caller's to decide, not this crate's. Most imports are named and
/// hashed - `Nid::with_suffix(name, suffix)` - but some arrive *already* as an identifier,
/// because firmware modules export around a million of them whose names nobody outside the
/// vendor holds. An import is perfectly resolvable without a name; the name only ever existed
/// to compute the identifier.
///
/// Handing back a [`Nid`] rather than letting this crate hash a string is what makes both
/// cases the same case, and it is why `build` takes no hash suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    /// The identifier a loader will match on.
    pub nid: Nid,
    /// Which library answers it.
    pub library: u16,
    /// Which module that library lives in.
    pub module: u16,
}

/// One library a module imports from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    /// Its name, as symbols encode it - `libkernel`, not `libkernel.prx`.
    pub name: String,
    /// The library id every symbol from it encodes.
    pub id: u16,
    /// The id of the module the library lives in.
    ///
    /// **Not the same number as `id`.** `libScePosix` is a library inside the `libkernel`
    /// module, and a writer that reuses one id for both produces symbols naming a module that
    /// does not exist.
    pub module_id: u16,
}

/// Where one table sits inside the built segment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Span {
    /// Offset from the start of the segment.
    pub at: u64,
    /// Size in bytes.
    pub size: u64,
}

impl Span {
    /// Whether the table holds anything.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }
}

/// The linked module's tables, as read out of its sections.
#[derive(Debug, Clone, Copy)]
pub struct Linked<'a> {
    /// `.dynsym` - the symbol entries, which keep their bindings, values and sizes.
    pub symbols: &'a [u8],
    /// The string table those entries name.
    pub names: &'a [u8],
    /// `.rela.plt`, copied unchanged.
    pub jmprel: &'a [u8],
    /// `.rela.dyn`, copied unchanged.
    pub rela: &'a [u8],
    /// Address of `.got.plt`, or `.got` when the linker did not split them, or zero.
    ///
    /// **An address, not an offset** - the only value here that is. A loader writes a resolved
    /// import into a slot measured from this base, so a wrong one puts every answer at the
    /// wrong place.
    pub pltgot: u64,
}

/// A built vendor segment and everything needed to describe it.
#[derive(Debug, Clone)]
pub struct Segment {
    /// The segment's bytes.
    pub bytes: Vec<u8>,
    /// The string table.
    pub strtab: Span,
    /// The symbol table.
    pub symtab: Span,
    /// The procedure-linkage relocations.
    pub jmprel: Span,
    /// The general relocations.
    pub rela: Span,
    /// The hash table.
    pub hash: Span,
    /// Where this module's own name sits in the string table.
    pub module_name_offset: u32,
    /// Each library, with the offsets of its bare name and its filename.
    pub libraries: Vec<(Library, u32, u32)>,
    /// The global offset table's address, carried through from [`Linked`].
    pub pltgot: u64,
    /// How many symbols were re-encoded as imports.
    pub encoded: usize,
}

/// Build the vendor segment from a linked module's tables.
///
/// `resolve` turns an undefined symbol's name into the [`Resolution`] a manifest holds for it.
/// Returning `None` means nobody claims it, and that is an error rather than a default: id
/// zero is a valid-looking answer that resolves to nothing.
///
/// **No hash suffix is passed in**, because the caller has already decided the identifier -
/// see [`Resolution`] for why that is the right place for the decision rather than a
/// convenience this crate should take over.
///
/// # Errors
///
/// If a table is malformed, if any undefined symbol is unclaimed, or if the string table would
/// exceed what a 32-bit offset can address.
pub fn build(
    linked: Linked<'_>,
    module_name: &str,
    libraries: &[Library],
    resolve: &dyn Fn(&str) -> Option<Resolution>,
) -> Result<Segment, BuildError> {
    let (mut strtab, module_name_offset, named) = start_strings(module_name, libraries)?;
    let (symbols, encoded) = rebuild_symbols(linked, &mut strtab, resolve)?;

    let hash = build_hash(&symbols, &strtab)?;

    // End to end, in the order a reference module uses. See the module docs: the adjacency
    // is what makes the tag arithmetic check out.
    //
    // The string table is **not** the first thing in the segment. A real executable reserves
    // `FINGERPRINT_SIZE` bytes ahead of it - sixteen bytes of build identifier and eight of
    // padding - and declares `DT_SCE_FINGERPRINT` with a value of zero, which is that region's
    // offset. Its string table then begins at `0x18`, and its own leading NUL sits there.
    //
    // Left out, the whole segment shifts down by `0x18` and every table lands where the
    // loader's layout calculation does not expect it. (D077)
    let fingerprint_len = usize::try_from(FINGERPRINT_SIZE).unwrap_or(0x18);
    let mut bytes = vec![0_u8; fingerprint_len];
    let strtab_span = Span {
        at: FINGERPRINT_SIZE,
        size: strtab.len() as u64,
    };
    bytes.extend_from_slice(&strtab);
    let symtab = append(&mut bytes, &symbols);
    let jmprel = append(&mut bytes, linked.jmprel);
    let rela = append(&mut bytes, linked.rela);
    let hash = append(&mut bytes, &hash);

    Ok(Segment {
        bytes,
        strtab: strtab_span,
        symtab,
        jmprel,
        rela,
        hash,
        module_name_offset,
        libraries: named,
        pltgot: linked.pltgot,
        encoded,
    })
}

impl Segment {
    /// Every dynamic entry describing this segment, in the order they must be emitted.
    ///
    /// `base` is zero when tag values are offsets into the segment and the segment's address
    /// when they are virtual addresses - which is the difference between the two conventions,
    /// and the reason one expression serves both.
    ///
    /// `generation` decides the version each library is declared at - almost always the
    /// default, and see [`module_version`] for the one case where it is not and what that
    /// costs.
    ///
    /// `init` is the module's initialiser address, if it defines one. Absent rather than zero
    /// when it does not: one loader calls the address unconditionally, so a zero there
    /// executes the ELF header as instructions.
    ///
    /// `kind` decides whether an export library is declared at all, and that is not a
    /// stylistic choice - see the export block below.
    #[must_use]
    pub fn entries(
        &self,
        table: Table,
        generation: Generation,
        base: u64,
        init: Option<u64>,
        kind: crate::ObjectType,
    ) -> Vec<(u64, u64)> {
        let tags = Tags::of(table);
        let at = |offset: u64| offset.saturating_add(base);

        // The string table first. Four tags below carry a name offset, and a loader resolves
        // one the moment it meets it.
        //
        // The fingerprint comes with it because it is what the string table's offset is
        // measured past - see [`FINGERPRINT_SIZE`]. Its value is the region's offset, which is
        // the start of the segment, so it is zero under the legacy convention and the
        // segment's own address under the current one.
        let mut entries = vec![
            (vendor::FINGERPRINT, at(0)),
            (tags.strtab, at(self.strtab.at)),
            (tags.strsz, self.strtab.size),
        ];

        entries.extend([
            (tags.symtab, at(self.symtab.at)),
            (tags.symtabsz, self.symtab.size),
            (tags.syment, SYMBOL_SIZE as u64),
            (tags.hash, at(self.hash.at)),
            (tags.hashsz, self.hash.size),
        ]);

        // The *form* is always declared; the table only when it holds something.
        //
        // The form is a statement about the format rather than about a table - "if there are
        // linkage relocations, they are `Elf64_Rela`" - and a loader that does not find it
        // concludes the module uses some other form and gives up.
        //
        // Declaring an empty table is the opposite mistake. It gives `JMPREL` and `RELA` the
        // same offset and a size of zero, and a loader handed that read relocations out of
        // the string table: entries whose "type" was four bytes of an encoded symbol name.
        entries.push((tags.pltrel, version::RELA_FORM));
        if !self.jmprel.is_empty() {
            entries.extend([
                (tags.jmprel, at(self.jmprel.at)),
                (tags.pltrelsz, self.jmprel.size),
            ]);
        }
        if !self.rela.is_empty() {
            entries.extend([
                (tags.rela, at(self.rela.at)),
                (tags.relasz, self.rela.size),
                (tags.relaent, RELA_SIZE as u64),
            ]);
        }
        if self.pltgot != 0 {
            entries.push((tags.pltgot, self.pltgot));
        }

        // Identity, last, because every value below packs a string-table offset.
        entries.extend([
            // The module's own name, which is the closest thing to a filename this knows.
            //
            // A real executable puts a whole build path here. Nothing in this repository has
            // one to put - a module is built from sources, not from a file with a name the
            // format cares about - so what goes in is the name the module already declares
            // for itself, which is true and is not invented. A loader needs the tag to be
            // present and its offset to resolve; it does not need the value to be a path.
            (
                vendor::ORIGINAL_FILENAME,
                u64::from(self.module_name_offset),
            ),
            (
                tags.module_info,
                module_identity(
                    0,
                    version::MODULE_MAJOR,
                    version::MODULE_MINOR,
                    self.module_name_offset,
                ),
            ),
            (tags.module_attr, 0),
        ]);

        // A main executable declares no export library, and that frees library id zero.
        //
        // # Why this is a correctness rule and not tidiness
        //
        // Export and import libraries share **one** id space. A shared library takes id zero
        // for the library it exports and numbers its imports from one, which is the ordinary
        // arrangement and is what this wrote unconditionally. An executable exports nothing,
        // so its first import library **is** id zero.
        //
        // Emitting the export tag on an executable therefore does two things, and the second
        // is the damaging one: it declares a library nothing imports, and it pushes every
        // import library up by one, so the table a loader indexes has no entry at zero. The
        // loader allocates that table densely from zero - `allocate_per_file_info_compact` is
        // the frame it refuses in - and a hole at the front is not a gap it tolerates.
        //
        // Settled by a launching homebrew executable: ten import libraries at ids 0 through 9,
        // nine needed modules at ids 1 through 9, and no export tag of either kind. (D074)
        if !kind.is_executable() {
            entries.extend([
                (
                    tags.export_lib,
                    identity(0, version::LIBRARY, self.module_name_offset),
                ),
                (vendor::EXPORT_LIB_ATTR, attribute(0, version::AUTO_EXPORT)),
            ]);
        }

        // Four entries per library. The ordinary `DT_NEEDED` names the module by **filename**
        // and the vendor tags name it by its bare name - `libkernel.prx` against `libkernel`.
        // A loader keying its implementation table on the filename finds nothing under the
        // bare one, so both strings are in the table and both are used.
        for (library, name_offset, file_offset) in &self.libraries {
            entries.push((standard::NEEDED, u64::from(*file_offset)));
            let (major, minor) = module_version(&library.name, generation);
            entries.push((
                tags.needed_module,
                module_identity(library.module_id, major, minor, *name_offset),
            ));
            entries.push((
                tags.import_lib,
                identity(library.id, version::LIBRARY, *name_offset),
            ));
            entries.push((
                vendor::IMPORT_LIB_ATTR,
                attribute(library.id, version::IMPORT_LIBRARY),
            ));
        }

        if let Some(address) = init {
            entries.push((standard::INIT, address));
        }
        entries
    }
}

/// A library paired with the offsets of its bare name and its filename.
type Named = Vec<(Library, u32, u32)>;

/// Start the string table with the names the identity tags refer to.
///
/// They go in before any symbol name so their offsets stay small and the head of the table
/// stays readable - which is the state it is usually read in.
fn start_strings(
    module_name: &str,
    libraries: &[Library],
) -> Result<(Vec<u8>, u32, Named), BuildError> {
    // A string table begins with a NUL so that offset zero means "no name".
    let mut strings = vec![0_u8];
    let module_name_offset = push(&mut strings, module_name)?;
    let mut named = Vec::with_capacity(libraries.len());
    for library in libraries {
        let bare = push(&mut strings, &library.name)?;
        let file = push(&mut strings, &format!("{}.prx", library.name))?;
        named.push((library.clone(), bare, file));
    }
    Ok((strings, module_name_offset, named))
}

/// Re-encode every imported symbol name and type it as a function.
fn rebuild_symbols(
    linked: Linked<'_>,
    strings: &mut Vec<u8>,
    resolve: &dyn Fn(&str) -> Option<Resolution>,
) -> Result<(Vec<u8>, usize), BuildError> {
    let mut out = Vec::with_capacity(linked.symbols.len());
    let mut encoded = 0_usize;
    let mut unclaimed: Vec<String> = Vec::new();

    let mut at = 0_usize;
    while at.saturating_add(SYMBOL_SIZE) <= linked.symbols.len() {
        let entry = linked
            .symbols
            .get(at..at.saturating_add(SYMBOL_SIZE))
            .ok_or(BuildError::MalformedSymbolTable)?;
        let mut rebuilt = entry.to_vec();

        let name_offset = read_u32(entry, 0)?;
        let offset = if name_offset == 0 {
            0
        } else {
            let plain = string_at(linked.names, name_offset);
            // Undefined symbols are the imports, and are what a loader resolves by hash.
            // Anything defined here keeps its plain name: nothing looks it up.
            let undefined = read_u16(entry, 6)? == 0 && !plain.is_empty();
            let claim = if undefined { resolve(&plain) } else { None };

            let written = if let Some(resolved) = claim {
                encoded = encoded.saturating_add(1);
                set_type(&mut rebuilt, FUNCTION)?;
                set_binding(&mut rebuilt, GLOBAL)?;
                selfish_nid::symbol_name(resolved.nid, resolved.library, resolved.module)
            } else {
                if undefined {
                    unclaimed.push(plain.clone());
                }
                plain
            };
            push(strings, &written)?
        };

        rebuilt
            .get_mut(..4)
            .ok_or(BuildError::MalformedSymbolTable)?
            .copy_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(&rebuilt);
        at = at.saturating_add(SYMBOL_SIZE);
    }

    if !unclaimed.is_empty() {
        return Err(BuildError::Unclaimed(unclaimed));
    }
    Ok((out, encoded))
}

/// Build the symbol hash table.
///
/// Bucket count is one per symbol - not the densest choice, and density is irrelevant for a
/// few hundred entries. Never zero, because a zero bucket count is a division by zero in
/// whatever walks it.
fn build_hash(symbols: &[u8], strings: &[u8]) -> Result<Vec<u8>, BuildError> {
    let count = symbols.len().checked_div(SYMBOL_SIZE).unwrap_or(0);
    let buckets = count.max(1);
    let mut bucket = vec![0_u32; buckets];
    let mut chain = vec![0_u32; count.max(1)];

    // From one, because index zero is the reserved null symbol.
    for index in 1..count {
        let at = index.checked_mul(SYMBOL_SIZE).ok_or(BuildError::TooLarge)?;
        let entry = symbols
            .get(at..at.saturating_add(SYMBOL_SIZE))
            .ok_or(BuildError::MalformedSymbolTable)?;
        let name = string_at(strings, read_u32(entry, 0)?);
        let slot = (elf_hash(name.as_bytes()) as usize)
            .checked_rem(buckets)
            .unwrap_or(0);
        // Standard chaining: the new entry takes the bucket head and points at whatever was
        // there, so a lookup walks the chain.
        let head = bucket.get(slot).copied().ok_or(BuildError::TooLarge)?;
        *chain.get_mut(index).ok_or(BuildError::TooLarge)? = head;
        *bucket.get_mut(slot).ok_or(BuildError::TooLarge)? =
            u32::try_from(index).map_err(|_| BuildError::TooLarge)?;
    }

    let mut out = Vec::with_capacity(8_usize.saturating_add(buckets.saturating_mul(8)));
    out.extend_from_slice(&u32::try_from(buckets).unwrap_or(1).to_le_bytes());
    out.extend_from_slice(&u32::try_from(chain.len()).unwrap_or(1).to_le_bytes());
    for value in bucket.iter().chain(chain.iter()) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    Ok(out)
}

/// The hash function from the ELF specification.
///
/// Not a choice - the format fixes it, and this is the one place in the repository where a
/// "reasonable" alternative would produce a table that looks right and finds nothing.
#[must_use]
pub fn elf_hash(name: &[u8]) -> u32 {
    let mut hash: u32 = 0;
    for byte in name {
        hash = hash.wrapping_shl(4).wrapping_add(u32::from(*byte));
        let high = hash & 0xF000_0000;
        if high != 0 {
            hash ^= high >> 24;
        }
        hash &= !high;
    }
    hash
}

/// Append a table, aligned, and say where it landed.
fn append(segment: &mut Vec<u8>, table: &[u8]) -> Span {
    // `next_multiple_of` rather than the modulus dance: a wrong offset here points a loader
    // at the middle of a table.
    let at = segment.len().next_multiple_of(8);
    segment.resize(at, 0);
    segment.extend_from_slice(table);
    Span {
        at: at as u64,
        size: table.len() as u64,
    }
}

/// Append a NUL-terminated string and return where it starts.
fn push(table: &mut Vec<u8>, value: &str) -> Result<u32, BuildError> {
    // `st_name` and the identity values both address the table with 32 bits. Truncating would
    // name a *different* string, which resolves to the wrong library rather than failing.
    let offset = u32::try_from(table.len()).map_err(|_| BuildError::TooLarge)?;
    table.extend_from_slice(value.as_bytes());
    table.push(0);
    Ok(offset)
}

/// Set a symbol's type, keeping its binding.
///
/// The binding is left alone deliberately: weak is what makes an unresolved import a null
/// address rather than a link failure, and that is load-bearing.
fn set_type(entry: &mut [u8], symbol_type: u8) -> Result<(), BuildError> {
    let info = entry.get_mut(4).ok_or(BuildError::MalformedSymbolTable)?;
    *info = (*info & 0xF0) | (symbol_type & 0x0F);
    Ok(())
}

/// The high nibble of `st_info`, leaving the type alone. See [`GLOBAL`].
fn set_binding(entry: &mut [u8], binding: u8) -> Result<(), BuildError> {
    let info = entry.get_mut(4).ok_or(BuildError::MalformedSymbolTable)?;
    *info = ((binding & 0x0F) << 4) | (*info & 0x0F);
    Ok(())
}

fn string_at(table: &[u8], at: u32) -> String {
    let Ok(at) = usize::try_from(at) else {
        return String::new();
    };
    let rest = table.get(at..).unwrap_or_default();
    let end = rest
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(rest.len());
    String::from_utf8_lossy(rest.get(..end).unwrap_or_default()).into_owned()
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, BuildError> {
    let mut out = [0_u8; 2];
    out.copy_from_slice(
        bytes
            .get(at..at.saturating_add(2))
            .ok_or(BuildError::MalformedSymbolTable)?,
    );
    Ok(u16::from_le_bytes(out))
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, BuildError> {
    let mut out = [0_u8; 4];
    out.copy_from_slice(
        bytes
            .get(at..at.saturating_add(4))
            .ok_or(BuildError::MalformedSymbolTable)?,
    );
    Ok(u32::from_le_bytes(out))
}

fn read_u64(bytes: &[u8], at: usize) -> Result<u64, BuildError> {
    let mut out = [0_u8; 8];
    out.copy_from_slice(
        bytes
            .get(at..at.saturating_add(8))
            .ok_or(BuildError::NotAModule)?,
    );
    Ok(u64::from_le_bytes(out))
}

/// Fit a built segment into a linked module, in place.
///
/// This is the surgery a linker cannot do: append the tables, point the declared vendor header
/// at them, overwrite the standard dynamic table with the vendor one, and remove the section
/// headers.
///
/// `init` is the module's initialiser address, looked up by the caller. Absent rather than
/// zero when there is none.
///
/// # The section headers go, and that is not tidying
///
/// The linked file carries `.rela.dyn` and `.rela.plt` as sections, and the appended segment
/// carries copies described by the vendor tags. **Two descriptions of the same relocations,
/// reachable two ways.** That is one too many: two separate loaders were observed reading
/// relocations from somewhere neither tag points at, logging entries whose "type" was four
/// bytes of ASCII out of a string table. A loader that mis-reads a relocation does not fail to
/// apply it - it applies it, writing a bad value at a bad address inside the loaded image.
///
/// Nothing is deleted. Only the header table that indexes it; the bytes stay where they are,
/// still described by the program headers, which is all a loader reads.
///
/// # Errors
///
/// If the module has no dynamic segment, no declared vendor header, or a dynamic table too
/// small for the entries the segment needs.
pub fn install(
    module: &mut Vec<u8>,
    segment: &Segment,
    table: Table,
    generation: Generation,
    init: Option<u64>,
) -> Result<Installed, BuildError> {
    // Appended past everything else, aligned so the segment starts somewhere a loader is
    // comfortable with.
    let padding = module
        .len()
        .next_multiple_of(16)
        .saturating_sub(module.len());
    module.resize(module.len().saturating_add(padding), 0);
    let segment_offset = module.len() as u64;
    module.extend_from_slice(&segment.bytes);

    let (dynamic_at, dynamic_size, dynamic_index, spare, base, kind) = {
        let elf = crate::Elf::parse(module).map_err(|_| BuildError::NotAModule)?;
        let dynamic = elf
            .segment(crate::segment::DYNAMIC)
            .ok_or(BuildError::NoDynamicSegment)?;
        let dynamic_index = elf
            .program_headers()
            .iter()
            .position(|header| header.p_type.get() == crate::segment::DYNAMIC)
            .ok_or(BuildError::NoDynamicSegment)?;
        let spare = elf
            .program_headers()
            .iter()
            .position(|header| header.p_type.get() == crate::segment::SCE_DYNLIBDATA)
            .ok_or(BuildError::NoVendorHeader)?;

        // Where the appended tables live in the address space, or zero when they do not.
        //
        // The two conventions differ here as much as they differ in tag numbers, and the two
        // halves go together. Legacy: the tables sit in a `PT_SCE_DYNLIBDATA` segment that is
        // never mapped, and every table tag holds an **offset into it**. Current: no such
        // segment appears in any retail dump - the tables are in the image and the tags hold
        // **virtual addresses**.
        let base = match table {
            Table::Legacy => 0,
            Table::Current => elf
                .program_headers()
                .iter()
                .filter(|header| header.p_type.get() == crate::segment::LOAD)
                .map(|header| header.vaddr.get().saturating_add(header.memsz.get()))
                .max()
                .unwrap_or(0)
                .next_multiple_of(crate::layout::ALLOCATION_GRANULARITY),
        };
        // What the module says it is, rather than what the caller believes - the export
        // decision below turns on it and the file is the only thing that cannot be out of
        // date about it.
        (
            dynamic.offset.get(),
            dynamic.filesz.get(),
            dynamic_index,
            spare,
            base,
            elf.object_type(),
        )
    };

    let entries = segment.entries(table, generation, base, init, kind);
    let dynamic_bytes = entries
        .len()
        .checked_add(1)
        .and_then(|slots| slots.checked_mul(16))
        .ok_or(BuildError::TooLarge)? as u64;

    // The dynamic table goes at the **tail of the vendor segment**, not in the image.
    //
    // # How the console addresses it, which is not how an ordinary system does
    //
    // On an ordinary system `PT_DYNAMIC` is a window onto a mapped `PT_LOAD` and the loader
    // reads it at its virtual address. A console executable does the opposite: its
    // `PT_DYNAMIC` carries **no address at all** and lies inside `PT_SCE_DYNLIBDATA`, which is
    // itself never mapped. The tables and the table of contents describing them are one
    // region, read out of the file together.
    //
    // The arithmetic in a real executable is exact rather than suggestive. Its vendor segment
    // runs `0x8c130 + 0x3760`, its dynamic table `0x8f450 + 0x440`, and both end at `0x8f890`
    // - the dynamic table is the last `0x440` bytes of the vendor segment, immediately after
    // the hash table, with nothing between them.
    //
    // That is also why the loader's frame is named the way it is. `preprocess_dt_entries` is
    // reached from `calcurate_sce_dynlibdata_layout`: walking the dynamic entries **is** how
    // the vendor blob's layout gets computed, because they live in it. With the table left
    // behind in a `PT_LOAD`, the loader walks the region it expects and finds no vendor tag
    // anywhere, then reports the first one it needed:
    //
    //     [rtld] ERROR preprocess_dt_entries:9589: does not have DT_SCE_SYMTABSZ or
    //            DT_SCE_HASHSZ tabs.
    //
    // which names two tags that were present and correct all along, a hundred kilobytes away
    // from where they were being looked for. (D076)
    clear_dynamic(module, dynamic_at, dynamic_size)?;
    let dynamic_offset = module.len() as u64;
    let grown = module
        .len()
        .checked_add(usize::try_from(dynamic_bytes).map_err(|_| BuildError::TooLarge)?)
        .ok_or(BuildError::TooLarge)?;
    module.resize(grown, 0);
    write_dynamic(module, dynamic_offset, &entries)?;

    let segment_size = segment
        .bytes
        .len()
        .checked_add(usize::try_from(dynamic_bytes).map_err(|_| BuildError::TooLarge)?)
        .ok_or(BuildError::TooLarge)? as u64;

    strip_sections(module)?;
    repurpose_header(module, spare, segment_offset, segment_size, base)?;
    place_dynamic(
        module,
        dynamic_index,
        dynamic_offset,
        dynamic_bytes,
        // Unmapped under the legacy convention, and part of the image under the current one,
        // for the same reason the vendor segment itself is.
        if base == 0 {
            0
        } else {
            base.saturating_add(dynamic_offset.saturating_sub(segment_offset))
        },
    )?;

    Ok(Installed {
        segment_offset,
        segment_size,
        table_base: base,
        tags: entries.len(),
        encoded: segment.encoded,
        libraries: segment.libraries.len(),
    })
}

/// What [`install`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Installed {
    /// Where the appended segment starts in the file.
    pub segment_offset: u64,
    /// How large it is.
    pub segment_size: u64,
    /// The address the tables were placed at, or zero when they are not mapped.
    pub table_base: u64,
    /// How many dynamic entries were written.
    pub tags: usize,
    /// How many symbols were re-encoded.
    pub encoded: usize,
    /// How many libraries the module imports from.
    pub libraries: usize,
}

/// Overwrite the dynamic table in place, terminated, with the remainder cleared.
fn write_dynamic(bytes: &mut [u8], offset: u64, entries: &[(u64, u64)]) -> Result<(), BuildError> {
    let mut at = usize::try_from(offset).map_err(|_| BuildError::TooLarge)?;
    for (tag, value) in entries {
        put_u64(bytes, at, *tag)?;
        put_u64(bytes, at.saturating_add(8), *value)?;
        at = at.saturating_add(16);
    }
    // A terminator, then zeroes. Leaving the linker's standard entries after it would have a
    // loader read tags this module no longer describes.
    put_u64(bytes, at, 0)?;
    put_u64(bytes, at.saturating_add(8), 0)?;
    Ok(())
}

/// Erase the table the linker left behind, where its own `PT_DYNAMIC` pointed.
///
/// The vendor table is written elsewhere now - see [`install`] - so what the linker produced
/// would otherwise stay in a mapped segment, describing sections that no longer have headers.
/// That is the same "two descriptions of one thing" that [`strip_sections`] exists to prevent,
/// and the cheaper half to remove.
///
/// Walks to the linker's own terminator rather than clearing the whole reservation. A linker
/// script that gives `.dynamic` the rest of its segment declares a `p_filesz` covering live
/// data, and zeroing all of that removes the module's `.got` along with its dynamic table.
fn clear_dynamic(bytes: &mut [u8], offset: u64, limit: u64) -> Result<(), BuildError> {
    let start = usize::try_from(offset).map_err(|_| BuildError::TooLarge)?;
    let limit = usize::try_from(limit).map_err(|_| BuildError::TooLarge)?;
    let mut at = start;
    while at.saturating_add(16) <= start.saturating_add(limit) {
        let tag = read_u64(bytes, at)?;
        let slot = bytes
            .get_mut(at..at.saturating_add(16))
            .ok_or(BuildError::TooLarge)?;
        slot.fill(0);
        if tag == 0 {
            break;
        }
        at = at.saturating_add(16);
    }
    Ok(())
}

/// Point `PT_DYNAMIC` at the table [`install`] wrote, and size it to exactly that.
///
/// Both halves matter. A real executable's `PT_DYNAMIC` is `0x440` bytes for sixty-seven tags
/// and a terminator - exact, not a reservation - and carries no address, because the table it
/// describes is in the file and never placed.
fn place_dynamic(
    bytes: &mut [u8],
    index: usize,
    offset: u64,
    size: u64,
    vaddr: u64,
) -> Result<(), BuildError> {
    let phoff = read_u64(bytes, 0x20)?;
    let at = usize::try_from(phoff)
        .ok()
        .and_then(|base| index.checked_mul(56).and_then(|by| base.checked_add(by)))
        .ok_or(BuildError::NoDynamicSegment)?;

    put_u32(bytes, at, crate::segment::DYNAMIC)?;
    put_u32(bytes, at.saturating_add(4), 0x4)?; // read-only
    put_u64(bytes, at.saturating_add(8), offset)?;
    put_u64(bytes, at.saturating_add(16), vaddr)?;
    put_u64(bytes, at.saturating_add(24), vaddr)?;
    put_u64(bytes, at.saturating_add(32), size)?;
    // Not zero, even with no address - unlike a vendor data segment.
    //
    // The rule that made `p_memsz` zero for `PT_SCE_DYNLIBDATA` is about *mappable* segments
    // asking to be placed at the null page. `PT_DYNAMIC` is not mapped by the loader at all,
    // and a real executable states `0x440` in both fields with an address of zero. Following
    // the vendor-segment rule here would state that the module has no dynamic table.
    put_u64(bytes, at.saturating_add(40), size)?;
    put_u64(bytes, at.saturating_add(48), 8)?;
    Ok(())
}

/// Remove the section header table. See [`install`] for why.
fn strip_sections(bytes: &mut [u8]) -> Result<(), BuildError> {
    // All four fields, not just the offset: a zeroed offset with a live count is a worse shape
    // than either a table or none, and a reader that trusts the count walks from zero.
    put_u64(bytes, 0x28, 0)?;
    put_u16(bytes, 0x3A, 0)?;
    put_u16(bytes, 0x3C, 0)?;
    put_u16(bytes, 0x3E, 0)?;
    Ok(())
}

/// Point the declared vendor header at the appended tables.
fn repurpose_header(
    bytes: &mut [u8],
    index: usize,
    offset: u64,
    size: u64,
    vaddr: u64,
) -> Result<(), BuildError> {
    let phoff = {
        let mut raw = [0_u8; 8];
        raw.copy_from_slice(bytes.get(0x20..0x28).ok_or(BuildError::NotAModule)?);
        u64::from_le_bytes(raw)
    };
    let at = usize::try_from(phoff)
        .ok()
        .and_then(|base| index.checked_mul(56).and_then(|by| base.checked_add(by)))
        .ok_or(BuildError::NoVendorHeader)?;

    // A vendor data segment, or an ordinary mapped one.
    //
    // With no address the tables are read out of the file and never placed, which is the
    // legacy shape. With one they are part of the image, which is what every retail
    // current-generation dump does - none of them carries a `PT_SCE_DYNLIBDATA` at all.
    // Read-only either way; nothing writes to a string table.
    let mapped = vaddr != 0;
    put_u32(
        bytes,
        at,
        if mapped {
            crate::segment::LOAD
        } else {
            crate::segment::SCE_DYNLIBDATA
        },
    )?;
    put_u32(bytes, at.saturating_add(4), 0x4)?; // read-only
    put_u64(bytes, at.saturating_add(8), offset)?;
    put_u64(bytes, at.saturating_add(16), vaddr)?;
    put_u64(bytes, at.saturating_add(24), vaddr)?;
    put_u64(bytes, at.saturating_add(32), size)?;
    // `p_memsz`, and **zero when the segment is not mapped**.
    //
    // This wrote `size` into both, which is right for a mapped segment and is an illegal header
    // for an unmapped one: a segment with no address and a non-zero memory size is asking to be
    // placed at address zero, the null page. A console's `rtld` refuses the file for it and names
    // the segment:
    //
    //     [rtld] ERROR scan_phdr:1164: B: error 8  i 5
    //     [rtld] ERROR _exec_self_imgact:1427: found illegal segment header in /app0/eboot.bin
    //
    // Index 5 is this header. Every vendor data segment in a real package's eboot -
    // `PT_SCE_DYNLIBDATA` and both of the `0x6FFFFFxx` pair - carries a file size and a memory
    // size of **zero**, which is the same statement the comment above already makes: the tables
    // are read out of the file and never placed. (measured)
    put_u64(bytes, at.saturating_add(40), if mapped { size } else { 0 })?;
    put_u64(
        bytes,
        at.saturating_add(48),
        if mapped {
            crate::layout::ALLOCATION_GRANULARITY
        } else {
            16
        },
    )?;
    Ok(())
}

fn put_u16(bytes: &mut [u8], at: usize, value: u16) -> Result<(), BuildError> {
    bytes
        .get_mut(at..at.saturating_add(2))
        .ok_or(BuildError::NotAModule)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put_u32(bytes: &mut [u8], at: usize, value: u32) -> Result<(), BuildError> {
    bytes
        .get_mut(at..at.saturating_add(4))
        .ok_or(BuildError::NotAModule)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn put_u64(bytes: &mut [u8], at: usize, value: u64) -> Result<(), BuildError> {
    bytes
        .get_mut(at..at.saturating_add(8))
        .ok_or(BuildError::NotAModule)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

/// What can go wrong building a segment.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    /// The symbol table is not a whole number of entries, or an entry is short.
    MalformedSymbolTable,
    /// Undefined symbols that no library claims.
    ///
    /// Named rather than counted, because the answer is always "add these to the manifest"
    /// and a count does not say which.
    Unclaimed(Vec<String>),
    /// A table grew past what a 32-bit offset can address.
    TooLarge,
    /// The bytes handed to [`install`] are not a readable module.
    NotAModule,
    /// The module has no `PT_DYNAMIC` segment to overwrite.
    NoDynamicSegment,
    /// The module declares no vendor segment header to repurpose.
    ///
    /// The linker script declares one pointing at a placeholder byte, precisely so there is a
    /// header to point at the tables afterwards. A linker drops a `PHDRS` entry with no
    /// section assigned to it, and a header that is absent cannot be repurposed later.
    NoVendorHeader,
    /// The reserved dynamic table is too small for the entries this module needs.
    ///
    /// Reported rather than written over whatever follows, so an under-sized reservation is a
    /// build failure and not a mystery. Every imported library costs four tags.
    DynamicTooSmall {
        /// Bytes the entries need.
        needed: u64,
        /// Bytes the linker script reserved.
        available: u64,
    },
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MalformedSymbolTable => write!(f, "the symbol table is malformed"),
            Self::Unclaimed(names) => {
                write!(f, "no library claims {} symbol(s):", names.len())?;
                for name in names.iter().take(8) {
                    write!(f, " {name}")?;
                }
                if names.len() > 8 {
                    write!(f, " and {} more", names.len().saturating_sub(8))?;
                }
                Ok(())
            }
            Self::TooLarge => write!(f, "a table grew past a 32-bit offset"),
            Self::NotAModule => write!(f, "not a readable module"),
            Self::NoDynamicSegment => write!(f, "the module has no dynamic segment"),
            Self::NoVendorHeader => write!(
                f,
                "the module declares no vendor segment header to repurpose"
            ),
            Self::DynamicTooSmall { needed, available } => write!(
                f,
                "the dynamic table needs {needed} bytes and the script reserved {available}"
            ),
        }
    }
}

impl std::error::Error for BuildError {}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{BuildError, Library, Linked, Resolution, build, elf_hash, module_version};
    use crate::dynamic::{self, Table, standard};
    use selfish_abi::Generation;

    /// A symbol table with one null entry and then the named ones.
    fn linked_symbols(names: &[(&str, u16)]) -> (Vec<u8>, Vec<u8>) {
        let mut strings = vec![0_u8];
        let mut symbols = vec![0_u8; 24];
        for (name, section) in names {
            let at = u32::try_from(strings.len()).unwrap();
            strings.extend_from_slice(name.as_bytes());
            strings.push(0);

            symbols.extend_from_slice(&at.to_le_bytes());
            symbols.push(0x10); // global binding, NOTYPE - what a linker leaves
            symbols.push(0);
            symbols.extend_from_slice(&section.to_le_bytes());
            symbols.extend_from_slice(&0_u64.to_le_bytes());
            symbols.extend_from_slice(&0_u64.to_le_bytes());
        }
        (symbols, strings)
    }

    /// Claim every symbol for library zero, hashing its name.
    ///
    /// Always `Some`: these tests are about what a claimed symbol becomes, and the unclaimed
    /// case has its own test.
    #[allow(
        clippy::unnecessary_wraps,
        reason = "it is the shape `build` takes, and matching it is the point"
    )]
    fn resolve_all(name: &str) -> Option<Resolution> {
        Some(Resolution {
            nid: selfish_nid::Nid::of(name),
            library: 0,
            module: 0,
        })
    }

    fn libraries() -> Vec<Library> {
        vec![Library {
            name: "libkernel".to_owned(),
            id: 0,
            module_id: 0,
        }]
    }

    #[test]
    fn what_is_written_reads_back_through_the_reader() {
        // Principle 4, and the reason the two sides are one crate.
        let (symbols, names) = linked_symbols(&[("sceKernelLoadStartModule", 0), ("local", 1)]);
        let segment = build(
            Linked {
                symbols: &symbols,
                names: &names,
                jmprel: &[],
                rela: &[],
                pltgot: 0x1000,
            },
            "probe",
            &libraries(),
            &resolve_all,
        )
        .expect("a segment");

        let entries = segment.entries(
            Table::Legacy,
            Generation::Previous,
            0,
            None,
            crate::ObjectType::SharedLibrary,
        );
        let info = dynamic::Info::from_entries(&entries);
        assert_eq!(info.table, Some(Table::Legacy));

        let imports = dynamic::imports(&segment.bytes, &info).expect("imports");
        assert_eq!(imports.len(), 1, "one undefined symbol, so one import");
        assert_eq!(imports[0].library, Some("libkernel"));
        assert_eq!(imports[0].module, Some("libkernel"));
        assert_eq!(
            imports[0].nid,
            selfish_nid::Nid::of("sceKernelLoadStartModule")
        );
    }

    #[test]
    fn a_defined_symbol_keeps_its_plain_name() {
        // Nothing looks it up by hash, and encoding it would lose the only name it has.
        let (symbols, names) = linked_symbols(&[("local", 1)]);
        let segment = build(
            Linked {
                symbols: &symbols,
                names: &names,
                jmprel: &[],
                rela: &[],
                pltgot: 0,
            },
            "probe",
            &libraries(),
            &resolve_all,
        )
        .expect("a segment");

        assert_eq!(segment.encoded, 0);
        let text = String::from_utf8_lossy(&segment.bytes);
        assert!(text.contains("local"), "the plain name survives");
    }

    #[test]
    fn an_unclaimed_import_is_an_error_and_is_named() {
        // Giving it library zero would be a valid-looking answer that resolves to nothing.
        let (symbols, names) = linked_symbols(&[("sceSomethingUnknown", 0)]);
        let error = build(
            Linked {
                symbols: &symbols,
                names: &names,
                jmprel: &[],
                rela: &[],
                pltgot: 0,
            },
            "probe",
            &libraries(),
            &|_| None,
        )
        .unwrap_err();
        assert_eq!(
            error,
            BuildError::Unclaimed(vec!["sceSomethingUnknown".to_owned()])
        );
    }

    #[test]
    fn an_import_is_typed_as_a_function() {
        // A linker leaves it NOTYPE. A console loader matches on hash *and type*, and an
        // untyped import binds to a stub that returns zero - so every call succeeds and
        // every answer is a plausible nothing.
        let (symbols, names) = linked_symbols(&[("sceKernelLoadStartModule", 0)]);
        let segment = build(
            Linked {
                symbols: &symbols,
                names: &names,
                jmprel: &[],
                rela: &[],
                pltgot: 0,
            },
            "probe",
            &libraries(),
            &resolve_all,
        )
        .expect("a segment");

        let entries = segment.entries(
            Table::Legacy,
            Generation::Previous,
            0,
            None,
            crate::ObjectType::SharedLibrary,
        );
        let info = dynamic::Info::from_entries(&entries);
        let read = dynamic::symbols(&segment.bytes, &info).expect("symbols");
        // Skipping index zero deliberately: the reserved null entry has section zero too, so
        // "is an import" alone finds it first and reports the null symbol's type.
        let import = read
            .iter()
            .find(|symbol| symbol.is_import() && symbol.name_offset != 0)
            .expect("an import");
        assert_eq!(import.kind(), super::FUNCTION);
        assert_eq!(import.binding(), 1, "and the binding is untouched");
    }

    #[test]
    fn the_string_table_is_declared_before_anything_that_names_a_string() {
        // A loader resolves a name offset the moment it meets one. Emitted first, these
        // dereference a base it does not have - a fault inside the loader, with nothing in
        // its log, before a guest instruction runs.
        let (symbols, names) = linked_symbols(&[("local", 1)]);
        let segment = build(
            Linked {
                symbols: &symbols,
                names: &names,
                jmprel: &[],
                rela: &[],
                pltgot: 0,
            },
            "probe",
            &libraries(),
            &resolve_all,
        )
        .expect("a segment");

        let entries = segment.entries(
            Table::Legacy,
            Generation::Previous,
            0,
            None,
            crate::ObjectType::SharedLibrary,
        );
        let tags = dynamic::Tags::of(Table::Legacy);
        let strtab = entries.iter().position(|(tag, _)| *tag == tags.strtab);
        let module_info = entries.iter().position(|(tag, _)| *tag == tags.module_info);
        // The fingerprint precedes it and names nothing; nothing else may.
        assert_eq!(
            entries.first().map(|(tag, _)| *tag),
            Some(dynamic::vendor::FINGERPRINT),
            "only the fingerprint comes before the string table"
        );
        assert_eq!(strtab, Some(1), "and the string table is next");
        assert!(module_info > strtab, "and every name comes after it");
    }

    #[test]
    fn an_empty_relocation_table_is_not_declared_but_its_form_always_is() {
        // Declaring an empty table gives JMPREL and RELA the same offset and a size of zero,
        // and one loader handed that read relocations out of the string table. The *form* is
        // a statement about the format rather than about a table, and a loader that does not
        // find it concludes the module uses some other form.
        let (symbols, names) = linked_symbols(&[("local", 1)]);
        let segment = build(
            Linked {
                symbols: &symbols,
                names: &names,
                jmprel: &[],
                rela: &[],
                pltgot: 0,
            },
            "probe",
            &libraries(),
            &resolve_all,
        )
        .expect("a segment");

        let entries = segment.entries(
            Table::Legacy,
            Generation::Previous,
            0,
            None,
            crate::ObjectType::SharedLibrary,
        );
        let tags = dynamic::Tags::of(Table::Legacy);
        assert!(entries.iter().any(|(tag, _)| *tag == tags.pltrel));
        assert!(!entries.iter().any(|(tag, _)| *tag == tags.jmprel));
        assert!(!entries.iter().any(|(tag, _)| *tag == tags.rela));
    }

    #[test]
    fn every_library_costs_four_tags() {
        // The number the linker script's reservation is sized by.
        let (symbols, names) = linked_symbols(&[("local", 1)]);
        let two = vec![
            Library {
                name: "libkernel".to_owned(),
                id: 0,
                module_id: 0,
            },
            Library {
                name: "libSceFios2".to_owned(),
                id: 1,
                module_id: 1,
            },
        ];
        let linked = Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        };
        let one = build(linked, "probe", &libraries(), &resolve_all)
            .expect("a segment")
            .entries(
                Table::Legacy,
                Generation::Previous,
                0,
                None,
                crate::ObjectType::SharedLibrary,
            )
            .len();
        let both = build(linked, "probe", &two, &resolve_all)
            .expect("a segment")
            .entries(
                Table::Legacy,
                Generation::Previous,
                0,
                None,
                crate::ObjectType::SharedLibrary,
            )
            .len();
        assert_eq!(both - one, crate::layout::TAGS_PER_LIBRARY);
    }

    #[test]
    fn the_two_conventions_produce_different_tags_for_the_same_segment() {
        // The bug this repository was founded on, in miniature: one segment, two tag sets,
        // and a builder that picked the wrong one produced a file a loader rejected.
        let (symbols, names) = linked_symbols(&[("local", 1)]);
        let segment = build(
            Linked {
                symbols: &symbols,
                names: &names,
                jmprel: &[],
                rela: &[],
                pltgot: 0,
            },
            "probe",
            &libraries(),
            &resolve_all,
        )
        .expect("a segment");

        let legacy = segment.entries(
            Table::Legacy,
            Generation::Previous,
            0,
            None,
            crate::ObjectType::SharedLibrary,
        );
        let current = segment.entries(
            Table::Current,
            Generation::Previous,
            0x1000,
            None,
            crate::ObjectType::SharedLibrary,
        );
        // The string table, because the fingerprint that now precedes it has one tag number
        // under both conventions - it is the *tables* that are renumbered.
        let find = |entries: &[(u64, u64)], table| {
            let wanted = dynamic::Tags::of(table).strtab;
            entries
                .iter()
                .find(|(tag, _)| *tag == wanted)
                .copied()
                .expect("a string table")
        };
        let (legacy_tag, legacy_value) = find(&legacy, Table::Legacy);
        let (current_tag, current_value) = find(&current, Table::Current);
        assert_ne!(legacy_tag, current_tag, "different tag numbers");
        assert_eq!(
            current_value - legacy_value,
            0x1000,
            "and the current convention's values are addresses"
        );
    }

    #[test]
    fn the_initialiser_is_absent_rather_than_zero_when_there_is_none() {
        // One loader calls the address without checking the tag was present. A zero there
        // executes the ELF header as instructions.
        let (symbols, names) = linked_symbols(&[("local", 1)]);
        let segment = build(
            Linked {
                symbols: &symbols,
                names: &names,
                jmprel: &[],
                rela: &[],
                pltgot: 0,
            },
            "probe",
            &libraries(),
            &resolve_all,
        )
        .expect("a segment");

        let without = segment.entries(
            Table::Legacy,
            Generation::Previous,
            0,
            None,
            crate::ObjectType::SharedLibrary,
        );
        let with = segment.entries(
            Table::Legacy,
            Generation::Previous,
            0,
            Some(0x2000),
            crate::ObjectType::SharedLibrary,
        );
        assert_eq!(with.len(), without.len() + 1);
        assert!(!without.iter().any(|(tag, _)| *tag == standard::INIT));
    }

    #[test]
    fn an_executable_declares_no_export_library_and_a_shared_library_does() {
        // Export and import libraries share one id space, so an export tag on an executable
        // costs library id zero - and the import table a loader indexes densely from zero
        // then has a hole at the front. See the export block in `entries`. (D074)
        let (symbols, names) = linked_symbols(&[("local", 1)]);
        let segment = build(
            Linked {
                symbols: &symbols,
                names: &names,
                jmprel: &[],
                rela: &[],
                pltgot: 0,
            },
            "probe",
            &libraries(),
            &resolve_all,
        )
        .expect("a segment");

        let exports = |kind| {
            segment
                .entries(Table::Legacy, Generation::Previous, 0, None, kind)
                .iter()
                .any(|(tag, _)| {
                    *tag == dynamic::vendor::EXPORT_LIB_LEGACY
                        || *tag == dynamic::vendor::EXPORT_LIB_ATTR
                })
        };
        assert!(
            !exports(crate::ObjectType::Executable),
            "an executable exports nothing"
        );
        assert!(
            !exports(crate::ObjectType::FixedExecutable),
            "and neither does a fixed one"
        );
        assert!(
            exports(crate::ObjectType::SharedLibrary),
            "a shared library is the whole reason the tag exists"
        );
    }

    #[test]
    fn a_library_is_named_twice_because_two_tags_spell_it_differently() {
        // `libkernel` for the vendor tags, `libkernel.prx` for DT_NEEDED. A loader keying its
        // implementation table on the filename finds nothing under the bare name.
        let (symbols, names) = linked_symbols(&[("local", 1)]);
        let segment = build(
            Linked {
                symbols: &symbols,
                names: &names,
                jmprel: &[],
                rela: &[],
                pltgot: 0,
            },
            "probe",
            &libraries(),
            &resolve_all,
        )
        .expect("a segment");

        let text = String::from_utf8_lossy(&segment.bytes);
        assert!(text.contains("libkernel\0"), "the bare name");
        assert!(text.contains("libkernel.prx\0"), "and the filename");
    }

    #[test]
    fn the_hash_is_the_one_the_specification_fixes() {
        // A "reasonable" alternative produces a table that looks right and finds nothing.
        assert_eq!(elf_hash(b""), 0);
        assert_eq!(elf_hash(b"printf"), 0x0779_05A6);
    }
    #[test]
    fn the_display_library_declares_a_different_version_on_the_previous_generation() {
        // The one row in `data/library-versions.tsv`, and the reason it is a table rather
        // than a constant. A loader matches the declared version against the registered one
        // and a mismatch resolves *nothing* from that library - silently. This library is the
        // one that decides whether anything appears on screen, so the symptom is a module
        // that runs perfectly and draws a black window.
        assert_eq!(
            module_version("libSceVideoOut", Generation::Previous),
            (0, 0)
        );
        assert_eq!(
            module_version("libSceVideoOut", Generation::Current),
            (1, 1)
        );
        assert_eq!(module_version("libkernel", Generation::Previous), (1, 1));
    }

    #[test]
    fn the_generation_reaches_the_library_entries() {
        // Threading it this far is the point: a version read correctly and then not used is
        // the same black window.
        let (symbols, names) = linked_symbols(&[("local", 1)]);
        let video = vec![Library {
            name: "libSceVideoOut".to_owned(),
            id: 0,
            module_id: 0,
        }];
        let segment = build(
            Linked {
                symbols: &symbols,
                names: &names,
                jmprel: &[],
                rela: &[],
                pltgot: 0,
            },
            "probe",
            &video,
            &resolve_all,
        )
        .expect("a segment");

        let tags = dynamic::Tags::of(Table::Legacy);
        let version_of = |generation| {
            segment
                .entries(
                    Table::Legacy,
                    generation,
                    0,
                    None,
                    crate::ObjectType::SharedLibrary,
                )
                .into_iter()
                .find(|(tag, _)| *tag == tags.needed_module)
                .map(|(_, value)| (value >> 32) & 0xFFFF)
                .expect("a needed-module entry")
        };
        assert_eq!(version_of(Generation::Previous), 0x0000, "0.0");
        assert_eq!(version_of(Generation::Current), 0x0101, "1.1");
    }

    /// An import library's attribute word is not the export one, and 0x1 is the bug.
    ///
    /// These two words are the same shape and were written by the same constant, which is how
    /// one value ended up standing for both. A module built with `0x1` on its import libraries
    /// loads and runs and binds nothing it had to load a library for - a failure that reads as
    /// the platform lacking the symbols, because from inside the module that is what it looks
    /// like.
    ///
    /// Pinned against the literal rather than against the constant: asserting
    /// `IMPORT_LIBRARY == IMPORT_LIBRARY` would pass no matter what the constant became, and
    /// the value is a measurement off a real title rather than a choice this crate is free to
    /// revise.
    #[test]
    fn an_import_librarys_attribute_is_not_the_export_attribute() {
        assert_eq!(
            super::version::IMPORT_LIBRARY,
            0x9,
            "as a launching title writes it"
        );
        assert_ne!(
            super::version::IMPORT_LIBRARY,
            super::version::AUTO_EXPORT,
            "one constant for both is the defect this test exists to catch"
        );
    }

    /// An import is re-bound GLOBAL, whatever the compiler marked it.
    ///
    /// A probe declares its platform functions weak so an absent one is null rather than a
    /// link error, and that binding used to travel straight into the module. A loader reads
    /// `STB_WEAK` undefined as "do not go to any trouble" and does exactly that: imports bound
    /// only from libraries already resident, and every library it would have had to load was
    /// mapped and left unbound.
    ///
    /// Pinned on the encoded byte rather than on the constants, because the whole defect was
    /// that the field nobody looked at held something nobody chose.
    #[test]
    fn an_import_is_rebound_global_even_when_the_compiler_marked_it_weak() {
        // st_info: binding in the high nibble, type in the low one. WEAK FUNC is 0x22.
        let mut entry = [0_u8; 24];
        entry[4] = 0x22;
        super::set_type(&mut entry, super::FUNCTION).expect("type");
        super::set_binding(&mut entry, super::GLOBAL).expect("binding");
        assert_eq!(
            entry[4], 0x12,
            "GLOBAL FUNC, as a launching title writes it"
        );
    }
}
