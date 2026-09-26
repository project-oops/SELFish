//! Building the vendor dynamic segment - the writing side of [`crate::dynamic`].
//!
//! The linker's tables are rebuilt and appended: the vendor segment holds them, imports are
//! renamed `<hash>#<library>#<module>` and typed as functions, and the dynamic table carries
//! vendor tags. [`crate::dynamic`] reads the result back, and tests round-trip the two.
//!
//! Which library resolves a name is the caller's manifest, passed as a closure.
//!
//! The string table is declared first, because a loader resolves each name offset as it meets
//! it. The tables are laid out end to end as in a reference module, so
//! `JMPREL + PLTRELSZ == RELA` and `RELA + RELASZ == HASH` hold for the output.

use selfish_abi::Generation;
use selfish_bytes::{read_le, write_le};
use selfish_nid::Nid;

use crate::dynamic::{Table, Tags, standard, vendor};
use crate::reloc::RELA_SIZE;
use crate::section::SYMBOL_SIZE;

/// `st_info` type for a function.
///
/// A linker leaves an import as `STT_NOTYPE`. The loader matches on hash and symbol type, and
/// binds a `NOTYPE` import to a stub that returns zero.
pub const FUNCTION: u8 = 2;

/// `st_info` binding for an import the loader is expected to bind.
///
/// A loader binds a `STB_WEAK` undefined import only from libraries already resident in the
/// process, so every import is rewritten `GLOBAL`, as a launching title carries them. A
/// module built here therefore has no weak undefined imports: a resolved one is `GLOBAL` and
/// an unresolved one is [`BuildError::Unclaimed`] (D101).
pub const GLOBAL: u8 = 1;

/// Version numbers a module and its libraries declare.
pub mod version {
    /// The default module version, major. See [`super::module_version`].
    pub const MODULE_MAJOR: u8 = 1;
    /// The default module version, minor.
    pub const MODULE_MINOR: u8 = 1;
    /// The version every platform library is registered with.
    ///
    /// A loader matches the version a module declares against the one the library was
    /// registered with; a mismatch resolves nothing from that library.
    pub const LIBRARY: u16 = 1;
    /// The library attribute meaning "export everything automatically".
    pub const AUTO_EXPORT: u64 = 0x1;
    /// The attribute word an import library carries, as a launching title writes it.
    ///
    /// With [`AUTO_EXPORT`] here instead, a loader binds imports only from libraries already
    /// resident. Bit 3's meaning is not established, so it is not given a name.
    pub const IMPORT_LIBRARY: u64 = 0x9;
    /// What `DT_SCE_PLTREL` states: the linkage relocations are `Elf64_Rela`.
    pub const RELA_FORM: u64 = 7;
}

/// Bytes reserved at the head of the vendor segment for the module's fingerprint.
///
/// Sixteen bytes of build identifier padded to `0x18`, then the string table.
/// `DT_SCE_FINGERPRINT` carries this region's offset, which is zero. Written as zeroes, like
/// every digest and signature area here; nothing is invented to fill it.
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
/// carries a row saying otherwise. A loader matches the declared version against the
/// registered one, and a mismatch resolves nothing from that library.
#[must_use]
pub fn module_version(library: &str, generation: Generation) -> (u8, u8) {
    let wanted = match generation {
        Generation::Prospero => "5",
        Generation::Orbis => "4",
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
/// The caller decides the identifier: most imports are hashed from a name with
/// `Nid::with_suffix`, and some are known only as an identifier. Taking a [`Nid`] covers both,
/// which is why `build` takes no hash suffix.
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
    /// Not necessarily `id`: `libScePosix` is a library inside the `libkernel` module.
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
    /// An address, not an offset: a loader writes resolved imports into slots measured from
    /// this base.
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
/// `None` means nobody claims it, which is an error rather than a default id. The caller has
/// already decided the identifier; see [`Resolution`].
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

    // End to end, in the order a reference module uses, after the fingerprint region; the
    // loader's layout calculation expects the string table at `FINGERPRINT_SIZE`.
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
    /// `base` is zero when tag values are offsets into the segment (Orbis) and the segment's
    /// address when they are virtual addresses (Prospero). `generation` picks each library's
    /// declared version; see [`module_version`].
    ///
    /// `init` is the initialiser address, omitted rather than zero when there is none, since a
    /// loader calls it unconditionally. `kind` decides whether an export library is declared.
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

        // The string table first, since later tags carry name offsets. The fingerprint's value
        // is the segment start: zero under Orbis, the segment's address under Prospero.
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

        // The form is always declared, since a loader without it gives up. A table is declared
        // only when non-empty; an empty one shares its offset with the next and a loader then
        // reads relocations out of the wrong table.
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
            // The module's own name. A loader needs the tag present and its offset to
            // resolve, not a path.
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

        // Export and import libraries share one id space. A shared library exports id zero
        // and imports from one; an executable exports nothing and its first import is id
        // zero. The loader allocates that table densely from zero and refuses a hole at the
        // front (`allocate_per_file_info_compact`).
        if !kind.is_executable() {
            entries.extend([
                (
                    tags.export_lib,
                    identity(0, version::LIBRARY, self.module_name_offset),
                ),
                (vendor::EXPORT_LIB_ATTR, attribute(0, version::AUTO_EXPORT)),
            ]);
        }

        // Four entries per library. `DT_NEEDED` names the filename (`libkernel.prx`) and the
        // vendor tags the bare name (`libkernel`).
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
/// They go in before any symbol name so they sit at the readable head of the table.
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

        let name_offset = read_le(entry, 0).ok_or(BuildError::MalformedSymbolTable)?;
        let offset = if name_offset == 0 {
            0
        } else {
            let plain = string_at(linked.names, name_offset)?;
            // Undefined symbols are the imports, and are what a loader resolves by hash.
            // Anything defined here keeps its plain name: nothing looks it up.
            let section = read_le::<u16>(entry, 6).ok_or(BuildError::MalformedSymbolTable)?;
            let undefined = section == 0 && !plain.is_empty();
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
/// One bucket per symbol, and never zero, since a reader divides by the bucket count.
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
        let name_offset = read_le(entry, 0).ok_or(BuildError::MalformedSymbolTable)?;
        let name = string_at(strings, name_offset)?;
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
/// The format fixes it; any other hash builds a table that finds nothing.
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
    // `st_name` and the identity values address the table with 32 bits; truncating would
    // name a different string.
    let offset = u32::try_from(table.len()).map_err(|_| BuildError::TooLarge)?;
    table.extend_from_slice(value.as_bytes());
    table.push(0);
    Ok(offset)
}

/// Set a symbol's type, keeping its binding. The binding is set by [`set_binding`].
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

/// A name from the string table this builder was handed.
///
/// Strict, because the name is written back. A lossy conversion would write a different
/// symbol name, and an out-of-range offset read as empty would make an undefined symbol look
/// defined and bypass [`BuildError::Unclaimed`]. Passing `.strtab` where `.dynstr` is wanted
/// puts every offset out of range (D091).
///
/// # Errors
///
/// If the offset is past the end of the table, if the name has no terminator inside it, or if
/// it is not UTF-8.
fn string_at(table: &[u8], at: u32) -> Result<String, BuildError> {
    let at = usize::try_from(at).map_err(|_| BuildError::SymbolName(at))?;
    let rest = table
        .get(at..)
        .ok_or_else(|| BuildError::SymbolName(u32::try_from(at).unwrap_or(u32::MAX)))?;
    let end = rest
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| BuildError::SymbolName(u32::try_from(at).unwrap_or(u32::MAX)))?;
    core::str::from_utf8(rest.get(..end).unwrap_or_default())
        .map(str::to_owned)
        .map_err(|_| BuildError::SymbolName(u32::try_from(at).unwrap_or(u32::MAX)))
}

/// Fit a built segment into a linked module, in place.
///
/// Appends the tables, points the declared vendor header at them, replaces the standard
/// dynamic table with the vendor one, and removes the section header table. `init` is the
/// initialiser address, or `None`.
///
/// The section headers go so the relocations are described only once, by the vendor tags; a
/// loader can otherwise read them from the wrong place. The section bytes stay in the file.
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
    // An unmapped Orbis vendor segment is 16-byte aligned; a mapped Prospero `PT_LOAD` needs
    // the allocation granularity.
    let align = match table {
        Table::Orbis => 16,
        Table::Prospero => usize::try_from(crate::layout::ALLOCATION_GRANULARITY).unwrap_or(0x4000),
    };
    let padding = module
        .len()
        .next_multiple_of(align)
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

        // Where the appended tables live in the address space, or zero when unmapped.
        let base = match table {
            Table::Orbis => 0,
            Table::Prospero => {
                let first_load_bias = elf
                    .program_headers()
                    .iter()
                    .find(|header| header.p_type.get() == crate::segment::LOAD)
                    .map_or(0, |header| {
                        header.offset.get().saturating_sub(header.vaddr.get())
                    });
                let max_va = elf
                    .program_headers()
                    .iter()
                    .filter(|header| header.p_type.get() == crate::segment::LOAD)
                    .map(|header| header.vaddr.get().saturating_add(header.memsz.get()))
                    .max()
                    .unwrap_or(0)
                    .next_multiple_of(crate::layout::ALLOCATION_GRANULARITY);
                max_va.max(segment_offset.saturating_sub(first_load_bias))
            }
        };
        // The export decision uses the type the file states, not what the caller believes.
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

    // The dynamic table is the tail of the vendor segment, right after the hash table, and
    // `PT_DYNAMIC` lies inside it. The loader computes the vendor segment's layout by walking
    // these entries; a table left in a `PT_LOAD` is reported as missing
    // `DT_SCE_SYMTABSZ or DT_SCE_HASHSZ`.
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
        // Unmapped under Orbis, part of the image under Prospero, like the vendor segment.
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
    // A terminator follows the entries.
    for (tag, value) in entries.iter().chain([&(0, 0)]) {
        write_le(bytes, at, *tag).ok_or(BuildError::NotAModule)?;
        write_le(bytes, at.saturating_add(8), *value).ok_or(BuildError::NotAModule)?;
        at = at.saturating_add(16);
    }
    Ok(())
}

/// Erase the table the linker left behind, where its own `PT_DYNAMIC` pointed.
///
/// The vendor table is written elsewhere by [`install`], so this one would be a second
/// description. Clears up to the linker's own terminator only, because `p_filesz` can cover
/// live data such as `.got`.
fn clear_dynamic(bytes: &mut [u8], offset: u64, limit: u64) -> Result<(), BuildError> {
    let start = usize::try_from(offset).map_err(|_| BuildError::TooLarge)?;
    let limit = usize::try_from(limit).map_err(|_| BuildError::TooLarge)?;
    let mut at = start;
    while at.saturating_add(16) <= start.saturating_add(limit) {
        let tag = read_le::<u64>(bytes, at).ok_or(BuildError::NotAModule)?;
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
/// Sized exactly, not as a reservation, and with no address under Orbis because the table is
/// never placed.
fn place_dynamic(
    bytes: &mut [u8],
    index: usize,
    offset: u64,
    size: u64,
    vaddr: u64,
) -> Result<(), BuildError> {
    let phoff = read_le::<u64>(bytes, 0x20).ok_or(BuildError::NotAModule)?;
    let at = usize::try_from(phoff)
        .ok()
        .and_then(|base| index.checked_mul(56).and_then(|by| base.checked_add(by)))
        .ok_or(BuildError::NoDynamicSegment)?;

    // `p_memsz` equals `p_filesz` even with no address, unlike a vendor data segment; a zero
    // would say the module has no dynamic table.
    let flags = if vaddr != 0 { 0x6 } else { 0x4 };
    write_program_header(
        bytes,
        at,
        (crate::segment::DYNAMIC, flags),
        [offset, vaddr, vaddr, size, size, 8],
    )
}

/// Write one 56-byte program header: type and flags, then offset, virtual and physical
/// address, file size, memory size and alignment.
fn write_program_header(
    bytes: &mut [u8],
    at: usize,
    (kind, flags): (u32, u32),
    fields: [u64; 6],
) -> Result<(), BuildError> {
    write_le(bytes, at, kind).ok_or(BuildError::NotAModule)?;
    write_le(bytes, at.saturating_add(4), flags).ok_or(BuildError::NotAModule)?;
    let mut field = at.saturating_add(8);
    for value in fields {
        write_le(bytes, field, value).ok_or(BuildError::NotAModule)?;
        field = field.saturating_add(8);
    }
    Ok(())
}

/// Remove the section header table. See [`install`] for why.
fn strip_sections(bytes: &mut [u8]) -> Result<(), BuildError> {
    // All four fields: a reader that trusts a live count walks from a zeroed offset.
    write_le(bytes, 0x28, 0_u64).ok_or(BuildError::NotAModule)?;
    for at in [0x3A, 0x3C, 0x3E] {
        write_le(bytes, at, 0_u16).ok_or(BuildError::NotAModule)?;
    }
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
    let phoff = read_le::<u64>(bytes, 0x20).ok_or(BuildError::NotAModule)?;
    let at = usize::try_from(phoff)
        .ok()
        .and_then(|base| index.checked_mul(56).and_then(|by| base.checked_add(by)))
        .ok_or(BuildError::NoVendorHeader)?;

    // With an address the tables are a read-only mapped segment, as in a Prospero-generation
    // executable. Without one they are an unplaced vendor data segment with a zero memory
    // size: `rtld` refuses an unmapped segment with a non-zero `p_memsz` as an illegal
    // segment header.
    let (kind, fields) = if vaddr != 0 {
        (
            (crate::segment::LOAD, 0x6),
            [
                offset,
                vaddr,
                vaddr,
                size,
                size,
                crate::layout::ALLOCATION_GRANULARITY,
            ],
        )
    } else {
        (
            (crate::segment::SCE_DYNLIBDATA, 0x4),
            [offset, vaddr, vaddr, size, 0, 16],
        )
    };
    write_program_header(bytes, at, kind, fields)
}

/// What can go wrong building a segment.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    /// The symbol table is not a whole number of entries, or an entry is short.
    MalformedSymbolTable,
    /// Undefined symbols that no library claims.
    ///
    /// Named rather than counted, so the caller knows what to add to the manifest.
    Unclaimed(Vec<String>),
    /// A symbol name could not be read from the string table it points into.
    ///
    /// Carries the offset of a name that is out of range, unterminated, or not UTF-8. The
    /// usual cause is passing `.strtab` where `.dynstr` is wanted.
    SymbolName(u32),
    /// A table grew past what a 32-bit offset can address.
    TooLarge,
    /// The bytes handed to [`install`] are not a readable module.
    NotAModule,
    /// The module has no `PT_DYNAMIC` segment to overwrite.
    NoDynamicSegment,
    /// The module declares no vendor segment header to repurpose.
    ///
    /// The linker script declares one over a placeholder byte, since a linker drops a `PHDRS`
    /// entry with no section assigned to it.
    NoVendorHeader,
    /// The reserved dynamic table is too small for the entries this module needs.
    ///
    /// Reported rather than written over whatever follows. Every imported library costs four
    /// tags.
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
            Self::SymbolName(at) => write!(
                f,
                "the symbol name at {at:#x} is out of range, unterminated or not UTF-8"
            ),
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
#[path = "dynlib_tests.rs"]
mod tests;
