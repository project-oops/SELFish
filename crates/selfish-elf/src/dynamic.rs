//! The dynamic table a loader actually reads.
//!
//! A vendor module carries two dynamic tables. The standard one is present and ignored; the
//! one that matters lives in `PT_SCE_DYNLIBDATA` and is described by tags whose values are
//! **offsets into that segment** rather than virtual addresses. Everything else about an
//! executable is preamble - this is where the string table, the symbol table, the
//! relocations and the library lists are.
//!
//! # Two conventions, and which one a module uses is not cosmetic
//!
//! [`Table::Legacy`] gives every table a vendor tag in the `0x6100_00xx` range.
//! [`Table::Current`] uses the **ordinary ELF numbers** for the standard tables and keeps
//! vendor tags only for the four things the standard has no name for. Both appear in the
//! wild; a loader that assumes one reads garbage from the other, and the garbage is
//! plausible - an offset is an offset.
//!
//! # The library list is not `DT_NEEDED`
//!
//! An encoded symbol name carries a library id, and those ids index the **vendor's** import
//! table rather than `DT_NEEDED`. The two are different lengths with different contents, and
//! indexing the wrong one produces attributions that fit and mean nothing: a graphics driver
//! appearing to export socket functions. Worth stating twice because it is the mistake this
//! module exists to make impossible.

use core::fmt;

use selfish_nid::Nid;

use crate::reloc::Rela;

/// Standard ELF dynamic tags, used verbatim by [`Table::Current`].
pub mod standard {
    /// A library this module needs.
    pub const NEEDED: u64 = 1;
    /// Size of the procedure-linkage relocations.
    pub const PLTRELSZ: u64 = 2;
    /// Address of the global offset table.
    pub const PLTGOT: u64 = 3;
    /// The symbol hash table.
    pub const HASH: u64 = 4;
    /// The string table.
    pub const STRTAB: u64 = 5;
    /// The symbol table.
    pub const SYMTAB: u64 = 6;
    /// The general relocations.
    pub const RELA: u64 = 7;
    /// Size of those.
    pub const RELASZ: u64 = 8;
    /// Size of one relocation entry.
    pub const RELAENT: u64 = 9;
    /// Size of the string table.
    pub const STRSZ: u64 = 10;
    /// Size of one symbol entry.
    pub const SYMENT: u64 = 11;
    /// A single initialisation function.
    pub const INIT: u64 = 12;
    /// Which relocation form the linkage table uses.
    pub const PLTREL: u64 = 20;
    /// The procedure-linkage relocations.
    pub const JMPREL: u64 = 23;
    /// An array of initialisation functions.
    pub const INIT_ARRAY: u64 = 25;
    /// Size of that array.
    pub const INIT_ARRAYSZ: u64 = 27;
    /// An array of pre-initialisation functions.
    pub const PREINIT_ARRAY: u64 = 32;
    /// Size of that array.
    pub const PREINIT_ARRAYSZ: u64 = 33;
}

/// Vendor dynamic tags.
pub mod vendor {
    /// Where the module's build fingerprint sits, which is the start of the vendor segment.
    ///
    /// The value is an offset like the table tags, and it is zero: the fingerprint occupies
    /// the head of the segment and the string table begins after it. See
    /// [`crate::dynlib::FINGERPRINT_SIZE`].
    ///
    /// This said "not emitted by anything here", which was true and was a gap rather than a
    /// decision - the region it points at is *structural*, and leaving it out moved every
    /// table in the segment.
    pub const FINGERPRINT: u64 = 0x6100_0007;
    /// The module's own filename, as an offset into the string table.
    ///
    /// A real executable puts the whole path its build produced here -
    /// `C:/Users/.../ORBIS_Debug/itemz_loader.elf` - which is a filename in the loosest sense
    /// and says more about the machine that built it than about the module.
    ///
    /// **Required for a shared library**, which is how it was found. A loader that is missing
    /// it counts what it has and refuses the file:
    ///
    /// ```text
    /// [rtld] ERROR preprocess_dt_entries:9600: C: orig fn 0  mod info 1
    /// ```
    ///
    /// - one module-info tag, and zero of these.
    pub const ORIGINAL_FILENAME: u64 = 0x6100_0009;
    /// The symbol hash table.
    pub const HASH: u64 = 0x6100_0025;
    /// Address of the global offset table. An address, not an offset.
    pub const PLTGOT: u64 = 0x6100_0027;
    /// The procedure-linkage relocations.
    pub const JMPREL: u64 = 0x6100_0029;
    /// Which relocation form the linkage table uses.
    pub const PLTREL: u64 = 0x6100_002B;
    /// Size of the procedure-linkage relocations.
    pub const PLTRELSZ: u64 = 0x6100_002D;
    /// The general relocations.
    pub const RELA: u64 = 0x6100_002F;
    /// Size of those.
    pub const RELASZ: u64 = 0x6100_0031;
    /// Size of one relocation entry.
    pub const RELAENT: u64 = 0x6100_0033;
    /// The string table.
    pub const STRTAB: u64 = 0x6100_0035;
    /// Size of the string table.
    pub const STRSZ: u64 = 0x6100_0037;
    /// The symbol table.
    pub const SYMTAB: u64 = 0x6100_0039;
    /// Size of one symbol entry.
    pub const SYMENT: u64 = 0x6100_003B;
    /// Size of the hash table.
    pub const HASHSZ: u64 = 0x6100_003D;
    /// Size of the whole symbol table.
    pub const SYMTABSZ: u64 = 0x6100_003F;

    /// This module's own name and version, in the legacy convention.
    ///
    /// # Two ranges, and the low one is not documented anywhere else
    ///
    /// The module and library tables have vendor numbers in **both** conventions, but not the
    /// same ones: the legacy convention puts them at `0x0D`-`0x19` and the current one at
    /// `0x43`-`0x49`. A reader written from retail material sees only the high range, and a
    /// writer targeting loaders emits only the low one - so each side documents half.
    ///
    /// This was found by reading a module that carries 352 entries at `0x6100_000F` and
    /// `0x6100_0015` with a table that only knew the high numbers, and reporting *zero*
    /// import libraries. Nothing was malformed; the reader simply looked in the wrong place
    /// and found nothing there, which is what a wrong tag number always looks like.
    pub const MODULE_INFO: u64 = 0x6100_000D;
    /// A module this one needs. Legacy convention.
    pub const NEEDED_MODULE_LEGACY: u64 = 0x6100_000F;
    /// Module attributes. Legacy convention.
    pub const MODULE_ATTR_LEGACY: u64 = 0x6100_0011;
    /// A library this module exports. Legacy convention.
    pub const EXPORT_LIB_LEGACY: u64 = 0x6100_0013;
    /// The library table an import's library id indexes. Legacy convention.
    pub const IMPORT_LIB_LEGACY: u64 = 0x6100_0015;
    /// Attributes of an exported library.
    pub const EXPORT_LIB_ATTR: u64 = 0x6100_0017;
    /// Attributes of an imported library.
    pub const IMPORT_LIB_ATTR: u64 = 0x6100_0019;

    /// This module's own name and version, in the current convention.
    pub const MODULE_INFO_CURRENT: u64 = 0x6100_0043;
    /// A module this one needs, indexed by an import's module id.
    ///
    /// One reader calls this `SCE_IMPORT_MODULE` and one writer calls it `NEEDED_MODULE`.
    /// Same tag, two names, and neither is wrong - recorded here rather than resolved,
    /// because a name is a reading and the number is the fact.
    pub const NEEDED_MODULE_CURRENT: u64 = 0x6100_0045;
    /// Module attributes.
    pub const MODULE_ATTR_CURRENT: u64 = 0x6100_0047;
    /// The library table an import's library id indexes.
    ///
    /// **Not `DT_NEEDED`.** Identified by counting: it holds exactly as many entries as
    /// there are distinct library ids, where `DT_NEEDED` does not.
    pub const IMPORT_LIB_CURRENT: u64 = 0x6100_0049;
    /// A library this module exports.
    ///
    /// **Not established for the current convention.** Retail main executables export
    /// nothing or one library and no tag for it was identified, so this value is the legacy
    /// one in both conventions rather than a guess.
    pub const EXPORT_LIB_CURRENT: u64 = 0x6100_004D;
}

/// The relocation form every module uses: `Elf64_Rela`.
pub const RELA_FORM: u64 = 7;

/// Size of one relocation entry, and of one symbol entry.
pub const ENTRY_SIZE: u64 = 0x18;

/// Which tag convention a module uses.
///
/// No `Default`, for the same reason [`selfish_abi::Generation`] has none: a caller that has
/// not decided has a question to answer rather than a value to omit, and the wrong choice
/// here is read as plausible offsets rather than as an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Table {
    /// Vendor tags for everything. What every loader examined accepts.
    Legacy,
    /// Standard tags for the standard tables, vendor tags for the vendor's own.
    ///
    /// Every number in this convention was read out of a retail module rather than chosen.
    Current,
}

/// The tag numbers for one convention, resolved together.
///
/// A struct rather than a function per tag, because a convention that is *half* applied is
/// worse than either whole one: it produces a table a loader parses successfully and reads
/// the wrong fields from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    missing_docs,
    reason = "each field is the tag its name says, per convention"
)]
pub struct Tags {
    pub strtab: u64,
    pub strsz: u64,
    pub symtab: u64,
    /// Unchanged between conventions: retail modules carry this vendor tag too.
    pub symtabsz: u64,
    pub syment: u64,
    pub hash: u64,
    /// Unchanged between conventions, as `symtabsz`.
    pub hashsz: u64,
    pub pltgot: u64,
    pub pltrelsz: u64,
    pub pltrel: u64,
    pub jmprel: u64,
    pub rela: u64,
    pub relasz: u64,
    pub relaent: u64,
    pub module_info: u64,
    pub needed_module: u64,
    pub module_attr: u64,
    pub import_lib: u64,
    pub export_lib: u64,
}

impl Tags {
    /// The numbers for a convention.
    #[must_use]
    pub const fn of(table: Table) -> Self {
        match table {
            Table::Legacy => Self {
                strtab: vendor::STRTAB,
                strsz: vendor::STRSZ,
                symtab: vendor::SYMTAB,
                symtabsz: vendor::SYMTABSZ,
                syment: vendor::SYMENT,
                hash: vendor::HASH,
                hashsz: vendor::HASHSZ,
                pltgot: vendor::PLTGOT,
                pltrelsz: vendor::PLTRELSZ,
                pltrel: vendor::PLTREL,
                jmprel: vendor::JMPREL,
                rela: vendor::RELA,
                relasz: vendor::RELASZ,
                relaent: vendor::RELAENT,
                module_info: vendor::MODULE_INFO,
                needed_module: vendor::NEEDED_MODULE_LEGACY,
                module_attr: vendor::MODULE_ATTR_LEGACY,
                import_lib: vendor::IMPORT_LIB_LEGACY,
                export_lib: vendor::EXPORT_LIB_LEGACY,
            },
            Table::Current => Self {
                strtab: standard::STRTAB,
                strsz: standard::STRSZ,
                symtab: standard::SYMTAB,
                symtabsz: vendor::SYMTABSZ,
                syment: standard::SYMENT,
                hash: standard::HASH,
                hashsz: vendor::HASHSZ,
                pltgot: standard::PLTGOT,
                pltrelsz: standard::PLTRELSZ,
                pltrel: standard::PLTREL,
                jmprel: standard::JMPREL,
                rela: standard::RELA,
                relasz: standard::RELASZ,
                relaent: standard::RELAENT,
                module_info: vendor::MODULE_INFO_CURRENT,
                needed_module: vendor::NEEDED_MODULE_CURRENT,
                module_attr: vendor::MODULE_ATTR_CURRENT,
                import_lib: vendor::IMPORT_LIB_CURRENT,
                export_lib: vendor::EXPORT_LIB_CURRENT,
            },
        }
    }

    /// Which convention a module is using, from the tags it actually carries.
    ///
    /// # The current convention is identified by its *identity* tags, not its string table
    ///
    /// The first version of this decided on the string table alone, reasoning that it is
    /// mandatory and that the two conventions number it unmistakably - `5` against
    /// `0x6100_0035`. Half of that is true. The legacy number is unmistakable; **`5` is also
    /// plain `DT_STRTAB`**, which every ordinary ELF on earth carries, so an unrelated shared
    /// object was reported as a current-convention vendor module.
    ///
    /// The current convention's identity tags - module info, needed module, import library -
    /// are all in the vendor range and cannot be confused with anything standard. They are
    /// what a current-convention module is recognised by. The string table stays as the
    /// legacy test, where it genuinely is unambiguous.
    ///
    /// `None` for a module that carries neither, which is the honest answer for an ordinary
    /// ELF rather than a coin toss between two conventions it uses neither of.
    #[must_use]
    pub fn detect(entries: &[(u64, u64)]) -> Option<Table> {
        let current = Self::of(Table::Current);
        for (tag, _) in entries {
            if *tag == vendor::STRTAB {
                return Some(Table::Legacy);
            }
            if *tag == current.module_info
                || *tag == current.needed_module
                || *tag == current.import_lib
            {
                return Some(Table::Current);
            }
        }
        None
    }
}

/// Splits a vendor table entry into its id and its name offset.
///
/// The value packs an id in the top sixteen bits, a version in the middle, and a string-table
/// offset in the bottom thirty-two.
#[must_use]
pub const fn split_table_entry(value: u64) -> (u16, u32) {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "both casts are of a masked or shifted field that fits by construction"
    )]
    ((value >> 48) as u16, (value & 0xFFFF_FFFF) as u32)
}

/// The dynamic table's contents.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info {
    /// Which convention the tags followed.
    pub table: Option<Table>,
    /// Offset or address of the string table.
    pub strtab: u64,
    /// Size of the string table.
    pub strsz: u64,
    /// Offset or address of the symbol table.
    pub symtab: u64,
    /// Size of the whole symbol table, where stated.
    pub symtabsz: u64,
    /// Size of one symbol entry.
    pub syment: u64,
    /// Offset or address of the hash table.
    pub hash: u64,
    /// The vendor's import-library table, as raw packed values.
    ///
    /// **Not `needed`.** An import's library id indexes this.
    pub import_libs: Vec<u64>,
    /// The vendor's module table, indexed by an import's module id.
    pub needed_modules: Vec<u64>,
    /// String-table offsets of the libraries `DT_NEEDED` lists.
    pub needed: Vec<u64>,
    /// The general relocations.
    pub rela: u64,
    /// Size of those.
    pub relasz: u64,
    /// The procedure-linkage relocations.
    pub jmprel: u64,
    /// Size of those.
    pub pltrelsz: u64,
    /// A single initialisation function, or zero.
    pub init: u64,
    /// An array of initialisation functions, or zero.
    pub init_array: u64,
    /// Size of that array.
    pub init_arraysz: u64,
}

impl Info {
    /// Read a dynamic table from its `(tag, value)` pairs.
    ///
    /// The convention is detected first and every lookup goes through it, so a module using
    /// standard numbering is never read with vendor tags or the reverse.
    #[must_use]
    pub fn from_entries(entries: &[(u64, u64)]) -> Self {
        let table = Tags::detect(entries);
        let tags = Tags::of(table.unwrap_or(Table::Legacy));
        let mut info = Self {
            table,
            syment: ENTRY_SIZE,
            ..Self::default()
        };
        for (tag, value) in entries.iter().copied() {
            match tag {
                t if t == tags.strtab => info.strtab = value,
                t if t == tags.strsz => info.strsz = value,
                t if t == tags.symtab => info.symtab = value,
                t if t == tags.symtabsz => info.symtabsz = value,
                t if t == tags.syment => info.syment = value,
                t if t == tags.hash => info.hash = value,
                t if t == tags.rela => info.rela = value,
                t if t == tags.relasz => info.relasz = value,
                t if t == tags.jmprel => info.jmprel = value,
                t if t == tags.pltrelsz => info.pltrelsz = value,
                t if t == tags.import_lib => info.import_libs.push(value),
                t if t == tags.needed_module => info.needed_modules.push(value),
                standard::NEEDED => info.needed.push(value),
                standard::INIT => info.init = value,
                standard::INIT_ARRAY => info.init_array = value,
                standard::INIT_ARRAYSZ => info.init_arraysz = value,
                _ => {}
            }
        }
        info
    }

    /// How many symbols the table holds, from whichever field states it.
    ///
    /// `symtabsz` where present, since it is exact. Otherwise `None` rather than a guess:
    /// a symbol count inferred from a table's extent is a count that silently changes when
    /// something else moves.
    #[must_use]
    pub fn symbol_count(&self) -> Option<u64> {
        if self.symtabsz == 0 || self.syment == 0 {
            return None;
        }
        self.symtabsz.checked_div(self.syment)
    }
}

/// Size of one symbol table entry.
pub const SYMBOL_SIZE: usize = 24;

/// One dynamic symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbol {
    /// Offset of its name in the string table.
    pub name_offset: u32,
    /// Binding and type, packed.
    pub info: u8,
    /// Visibility.
    pub other: u8,
    /// Section index. Zero means undefined - which for these modules means *imported*.
    pub section: u16,
    /// Address, or zero for an import.
    pub value: u64,
    /// Size, where stated.
    pub size: u64,
}

impl Symbol {
    /// Whether this symbol is defined elsewhere, and therefore imported.
    ///
    /// Section index zero. That is the whole test, and it is the one that separates the
    /// thousands of names a module asks for from the handful it provides.
    #[must_use]
    pub const fn is_import(&self) -> bool {
        self.section == 0
    }

    /// The binding, from the high nibble of `info`.
    #[must_use]
    pub const fn binding(&self) -> u8 {
        self.info >> 4
    }

    /// The type, from the low nibble.
    #[must_use]
    pub const fn kind(&self) -> u8 {
        self.info & 0xF
    }
}

/// Read the symbol table out of a vendor segment.
///
/// Offsets in the dynamic table are relative to the segment, not to the file, which is the
/// distinction that makes reading these tables from a file offset produce plausible garbage.
///
/// # Errors
///
/// If the table runs past the end of the segment.
pub fn symbols(segment: &[u8], info: &Info) -> Result<Vec<Symbol>, DynamicError> {
    let entry = usize::try_from(info.syment).unwrap_or(SYMBOL_SIZE);
    if entry == 0 {
        return Err(DynamicError::MalformedTable("symbol entry size is zero"));
    }
    let base = usize::try_from(info.symtab).map_err(|_| DynamicError::TableOutOfRange)?;

    // Bounded by whichever is smaller: what the table says it holds, and what is actually
    // there. A stated size is a claim, and one larger than the segment asks for an allocation
    // sized by a file.
    let available = segment.len().saturating_sub(base);
    let stated = usize::try_from(info.symtabsz).unwrap_or(0);
    let span = if stated == 0 || stated > available {
        available
    } else {
        stated
    };

    let mut out = Vec::with_capacity(span.checked_div(entry).unwrap_or(0));
    let mut at = base;
    let end = base
        .checked_add(span)
        .ok_or(DynamicError::TableOutOfRange)?;
    while at.saturating_add(SYMBOL_SIZE) <= end {
        let raw = segment
            .get(at..at.saturating_add(SYMBOL_SIZE))
            .ok_or(DynamicError::TableOutOfRange)?;
        out.push(Symbol {
            name_offset: read_u32(raw, 0)?,
            info: raw.get(4).copied().unwrap_or(0),
            other: raw.get(5).copied().unwrap_or(0),
            section: read_u16(raw, 6)?,
            value: read_u64(raw, 8)?,
            size: read_u64(raw, 16)?,
        });
        at = at.saturating_add(entry);
    }
    Ok(out)
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, DynamicError> {
    let raw = bytes
        .get(at..at.saturating_add(2))
        .ok_or(DynamicError::TableOutOfRange)?;
    let mut out = [0_u8; 2];
    out.copy_from_slice(raw);
    Ok(u16::from_le_bytes(out))
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, DynamicError> {
    let raw = bytes
        .get(at..at.saturating_add(4))
        .ok_or(DynamicError::TableOutOfRange)?;
    let mut out = [0_u8; 4];
    out.copy_from_slice(raw);
    Ok(u32::from_le_bytes(out))
}

fn read_u64(bytes: &[u8], at: usize) -> Result<u64, DynamicError> {
    let raw = bytes
        .get(at..at.saturating_add(8))
        .ok_or(DynamicError::TableOutOfRange)?;
    let mut out = [0_u8; 8];
    out.copy_from_slice(raw);
    Ok(u64::from_le_bytes(out))
}

/// An import, resolved as far as the module itself can resolve it.
///
/// The names are what the *importing* module claims. A loader still has to find a module of
/// that name and a symbol of that hash inside it; this is the question, not the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Import<'a> {
    /// Position in the dynamic symbol table.
    ///
    /// Carried because a relocation names its symbol by index and by nothing else, so this
    /// is the only thing that joins the two tables. Recovering it afterwards means walking
    /// the symbol table a second time and hoping the filtering matched.
    pub index: u32,
    /// The hash the loader looks up.
    pub nid: Nid,
    /// The library it should be looked up in, if the library table names one.
    pub library: Option<&'a str>,
    /// The module that library lives in, if the module table names one.
    pub module: Option<&'a str>,
    /// The underlying symbol, for anything the resolved view drops.
    pub symbol: Symbol,
}

/// The string table, as a slice of the segment holding it.
///
/// Empty rather than an error when the table says nothing, because a module with no strings
/// is unusual but not malformed.
#[must_use]
pub fn strings<'a>(segment: &'a [u8], info: &Info) -> &'a [u8] {
    let base = usize::try_from(info.strtab).unwrap_or(0);
    let size = usize::try_from(info.strsz).unwrap_or(0);
    let rest = segment.get(base..).unwrap_or_default();
    match rest.get(..size) {
        // A stated size larger than what is there is a claim the file does not back, and the
        // rest of the segment is the honest answer.
        Some(exact) if size > 0 => exact,
        _ => rest,
    }
}

/// Every symbol this module imports, with its library and module named where they can be.
///
/// Undefined symbols only - a module's own definitions are in the same table and are not
/// imports. That test is the section index, and skipping it reports a module as importing
/// everything it exports.
///
/// # Errors
///
/// If the symbol table runs past the end of the segment.
pub fn imports<'a>(segment: &'a [u8], info: &Info) -> Result<Vec<Import<'a>>, DynamicError> {
    let table = strings(segment, info);
    let mut out = Vec::new();
    for (index, symbol) in symbols(segment, info)?.into_iter().enumerate() {
        if !symbol.is_import() {
            continue;
        }
        let Ok(name) = string_at(table, symbol.name_offset) else {
            continue;
        };
        // A name that does not carry ids is a plain undefined symbol rather than a vendor
        // import - real modules hold both, and treating the difference as an error stops
        // the walk on the first one.
        let Some(encoded) = selfish_nid::decode_symbol_name(name) else {
            continue;
        };
        out.push(Import {
            index: u32::try_from(index).unwrap_or(u32::MAX),
            nid: encoded.nid,
            library: named(table, &info.import_libs, encoded.library_id),
            module: named(table, &info.needed_modules, encoded.module_id),
            symbol,
        });
    }
    Ok(out)
}

/// Look a packed table entry up by the id it carries.
///
/// **By id, not by position.** The ids are the module's own numbering and nothing promises
/// they are dense or in order; indexing the vector directly reads the wrong name whenever
/// they are not, and reads a plausible one, which is worse.
fn named<'a>(strings: &'a [u8], table: &[u64], id: u16) -> Option<&'a str> {
    let offset = table.iter().find_map(|packed| {
        let (entry_id, offset) = split_table_entry(*packed);
        (entry_id == id).then_some(offset)
    })?;
    string_at(strings, offset).ok()
}

/// The name of a dynamic tag, where this crate has one.
///
/// Here rather than in whatever prints it, so the constants and the names a reader sees
/// cannot drift apart. **A printer with its own table is a table that goes stale**, and a
/// stale one reports a tag that has been assigned as unknown - which reads as an open
/// question that was in fact answered.
///
/// `None` rather than a placeholder, for the same reason [`crate::reloc::kind::name`] returns
/// one: an unnamed tag is a thing to go and look up, and a plausible label is how it stops
/// being noticed.
///
/// Only the vendor tags and `DT_NEEDED` are named. The standard numbers are shared with the
/// current convention's table tags - `5` is both `DT_STRTAB` and the current convention's
/// string-table tag - so naming them here would print one meaning for a number that has two.
#[must_use]
pub const fn tag_name(tag: u64) -> Option<&'static str> {
    Some(match tag {
        standard::NEEDED => "DT_NEEDED",
        vendor::FINGERPRINT => "DT_SCE_FINGERPRINT",
        vendor::HASH => "DT_SCE_HASH",
        vendor::PLTGOT => "DT_SCE_PLTGOT",
        vendor::PLTRELSZ => "DT_SCE_PLTRELSZ",
        vendor::PLTREL => "DT_SCE_PLTREL",
        vendor::JMPREL => "DT_SCE_JMPREL",
        vendor::RELA => "DT_SCE_RELA",
        vendor::RELASZ => "DT_SCE_RELASZ",
        vendor::RELAENT => "DT_SCE_RELAENT",
        vendor::STRTAB => "DT_SCE_STRTAB",
        vendor::STRSZ => "DT_SCE_STRSZ",
        vendor::SYMTAB => "DT_SCE_SYMTAB",
        vendor::SYMENT => "DT_SCE_SYMENT",
        vendor::HASHSZ => "DT_SCE_HASHSZ",
        vendor::SYMTABSZ => "DT_SCE_SYMTABSZ",
        vendor::MODULE_INFO | vendor::MODULE_INFO_CURRENT => "DT_SCE_MODULE_INFO",
        vendor::NEEDED_MODULE_LEGACY | vendor::NEEDED_MODULE_CURRENT => "DT_SCE_NEEDED_MODULE",
        vendor::MODULE_ATTR_LEGACY | vendor::MODULE_ATTR_CURRENT => "DT_SCE_MODULE_ATTR",
        vendor::EXPORT_LIB_LEGACY => "DT_SCE_EXPORT_LIB",
        vendor::IMPORT_LIB_LEGACY | vendor::IMPORT_LIB_CURRENT => "DT_SCE_IMPORT_LIB",
        vendor::EXPORT_LIB_ATTR => "DT_SCE_EXPORT_LIB_ATTR",
        vendor::IMPORT_LIB_ATTR => "DT_SCE_IMPORT_LIB_ATTR",
        _ => return None,
    })
}

/// A name from the string table.
///
/// # Errors
///
/// If the offset is past the end, or the name is not terminated within it.
pub fn string_at(strings: &[u8], offset: u32) -> Result<&str, DynamicError> {
    let at = usize::try_from(offset).map_err(|_| DynamicError::StringOutOfRange(offset))?;
    let rest = strings
        .get(at..)
        .ok_or(DynamicError::StringOutOfRange(offset))?;
    let end = rest
        .iter()
        .position(|b| *b == 0)
        .ok_or(DynamicError::UnterminatedString(offset))?;
    core::str::from_utf8(rest.get(..end).unwrap_or_default())
        .map_err(|_| DynamicError::StringNotUtf8(offset))
}

/// Why a dynamic table could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicError {
    /// A string offset is past the end of the table.
    StringOutOfRange(u32),
    /// A string runs to the end of the table without a terminator.
    UnterminatedString(u32),
    /// A string is not valid UTF-8.
    StringNotUtf8(u32),
    /// A table runs past the end of the segment holding it.
    TableOutOfRange,
    /// A table describes something impossible.
    MalformedTable(&'static str),
}

impl fmt::Display for DynamicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StringOutOfRange(at) => {
                write!(f, "string offset {at:#x} is past the end of the table")
            }
            Self::UnterminatedString(at) => {
                write!(f, "the string at {at:#x} is not terminated")
            }
            Self::StringNotUtf8(at) => write!(f, "the string at {at:#x} is not UTF-8"),
            Self::TableOutOfRange => write!(f, "a table runs past the end of its segment"),
            Self::MalformedTable(what) => write!(f, "malformed table: {what}"),
        }
    }
}

impl std::error::Error for DynamicError {}

/// The two relocation tables, kept apart.
///
/// A named pair rather than a tuple, because the two are applied differently and swapping
/// them produces an image that relocates cleanly and jumps to the wrong place.
#[derive(Debug, Clone, Default)]
pub struct Relocations {
    /// `DT_RELA` - data relocations.
    pub data: Vec<Rela>,
    /// `DT_JMPREL` - the procedure linkage table, one slot per imported function.
    pub plt: Vec<Rela>,
}

/// Read both relocation tables out of a vendor segment.
///
/// Offsets are relative to the segment, as everywhere else in this table. A range that runs
/// past the end yields an empty table rather than an error: the sizes come from the same
/// tags, and a module with a stale one is still worth reading.
#[must_use]
pub fn relocations(segment: &[u8], info: &Info) -> Relocations {
    Relocations {
        data: crate::reloc::table(span(segment, info.rela, info.relasz)),
        plt: crate::reloc::table(span(segment, info.jmprel, info.pltrelsz)),
    }
}

fn span(segment: &[u8], at: u64, size: u64) -> &[u8] {
    let (Ok(at), Ok(size)) = (usize::try_from(at), usize::try_from(size)) else {
        return &[];
    };
    let end = at.saturating_add(size);
    segment.get(at..end).unwrap_or_default()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{
        DynamicError, Info, SYMBOL_SIZE, Table, Tags, imports, split_table_entry, standard,
        string_at, symbols, vendor,
    };

    #[test]
    fn the_two_conventions_disagree_where_it_matters_and_agree_where_it_does_not() {
        let legacy = Tags::of(Table::Legacy);
        let current = Tags::of(Table::Current);

        // The standard tables get different numbers.
        assert_ne!(legacy.strtab, current.strtab);
        assert_ne!(legacy.symtab, current.symtab);
        assert_ne!(legacy.rela, current.rela);

        // The vendor's own tables get vendor tags in both - but **not the same ones**. This
        // assertion originally read `assert_eq!`, which is the bug this file was written
        // with: the low range belongs to the legacy convention and the high range to the
        // current one, and treating them as shared reports zero import libraries on a
        // module carrying three hundred and fifty-two.
        assert_ne!(legacy.import_lib, current.import_lib);
        assert_ne!(legacy.module_info, current.module_info);
        assert_ne!(legacy.needed_module, current.needed_module);

        // Two genuinely are shared, and both were read from retail material carrying them
        // alongside standard tags.
        assert_eq!(legacy.symtabsz, current.symtabsz);
        assert_eq!(legacy.hashsz, current.hashsz);
    }

    #[test]
    fn the_string_table_alone_identifies_only_the_legacy_convention() {
        // This test used to assert that `standard::STRTAB` means the current convention, and
        // that assertion **was the bug**: `5` is also plain `DT_STRTAB`, so it reported every
        // ordinary shared object as a vendor module. Corrected rather than deleted, so the
        // distinction it now states cannot be quietly re-lost.
        assert_eq!(
            Tags::detect(&[(vendor::STRTAB, 0x100)]),
            Some(Table::Legacy),
            "the legacy number is in the vendor range and means only one thing"
        );
        assert_eq!(
            Tags::detect(&[(standard::STRTAB, 0x100)]),
            None,
            "the standard number means nothing on its own"
        );
        // No string table at all is not a convention to guess at.
        assert_eq!(Tags::detect(&[(standard::NEEDED, 1)]), None);
    }

    #[test]
    fn a_current_convention_table_is_not_read_with_vendor_tags() {
        // The failure this exists to prevent. Read with the wrong convention, every table
        // address comes back zero and the module looks empty rather than misparsed.
        let entries = [
            (standard::STRTAB, 0x1000),
            (standard::SYMTAB, 0x2000),
            (standard::RELA, 0x3000),
            (vendor::IMPORT_LIB_CURRENT, 0x0001_0000_0000_0020),
        ];
        let info = Info::from_entries(&entries);
        assert_eq!(info.table, Some(Table::Current));
        assert_eq!(info.strtab, 0x1000);
        assert_eq!(info.symtab, 0x2000);
        assert_eq!(info.rela, 0x3000);
        assert_eq!(info.import_libs.len(), 1);
    }

    #[test]
    fn a_legacy_table_reads_the_same_fields_from_different_numbers() {
        let entries = [
            (vendor::STRTAB, 0x1000),
            (vendor::SYMTAB, 0x2000),
            (vendor::RELA, 0x3000),
        ];
        let info = Info::from_entries(&entries);
        assert_eq!(info.table, Some(Table::Legacy));
        assert_eq!(info.strtab, 0x1000);
        assert_eq!(info.symtab, 0x2000);
        assert_eq!(info.rela, 0x3000);
    }

    #[test]
    fn the_import_library_table_is_kept_apart_from_needed() {
        // The mistake this module exists to make impossible: an import's library id indexes
        // the vendor table, and indexing DT_NEEDED instead produces attributions that fit
        // and mean nothing.
        let entries = [
            (vendor::STRTAB, 0x1000),
            (standard::NEEDED, 0x10),
            (standard::NEEDED, 0x20),
            (vendor::IMPORT_LIB_LEGACY, 0x0002_0000_0000_0030),
        ];
        let info = Info::from_entries(&entries);
        assert_eq!(info.needed.len(), 2, "DT_NEEDED entries");
        assert_eq!(info.import_libs.len(), 1, "vendor import libraries");
        assert_ne!(
            info.needed.len(),
            info.import_libs.len(),
            "the two lists are different, which is the whole point"
        );
    }

    #[test]
    fn a_packed_table_entry_splits_into_an_id_and_a_name_offset() {
        let (id, offset) = split_table_entry(0x0007_0001_0000_1234);
        assert_eq!(id, 7);
        assert_eq!(offset, 0x1234);
    }

    #[test]
    fn the_symbol_count_comes_from_a_stated_size_or_nowhere() {
        let stated = Info {
            syment: 0x18,
            symtabsz: 0x18 * 5,
            ..Info::default()
        };
        assert_eq!(stated.symbol_count(), Some(5));

        // Not stated is not a licence to infer one.
        let unstated = Info {
            syment: 0x18,
            ..Info::default()
        };
        assert_eq!(unstated.symbol_count(), None);
    }

    #[test]
    fn strings_are_read_only_where_they_are_actually_terminated() {
        let table = b"\0libSceNet\0libkernel\0";
        assert_eq!(string_at(table, 1), Ok("libSceNet"));
        assert_eq!(string_at(table, 11), Ok("libkernel"));
        assert_eq!(string_at(table, 0), Ok(""));
        assert_eq!(
            string_at(table, 999),
            Err(DynamicError::StringOutOfRange(999))
        );
        assert_eq!(
            string_at(b"no terminator", 0),
            Err(DynamicError::UnterminatedString(0))
        );
    }
    /// Build a vendor segment holding a string table and a symbol table.
    ///
    /// Laid out the way a real one is: strings first, symbols after, offsets relative to the
    /// segment rather than to the file.
    fn segment(names: &[&str]) -> (Vec<u8>, Vec<u32>) {
        let mut bytes = vec![0_u8];
        let mut offsets = Vec::new();
        for name in names {
            offsets.push(u32::try_from(bytes.len()).expect("a small table"));
            bytes.extend_from_slice(name.as_bytes());
            bytes.push(0);
        }
        (bytes, offsets)
    }

    /// Append one symbol table entry.
    fn push_symbol(bytes: &mut Vec<u8>, name_offset: u32, section: u16) {
        bytes.extend_from_slice(&name_offset.to_le_bytes());
        bytes.push(0x12); // global binding, function type
        bytes.push(0);
        bytes.extend_from_slice(&section.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
    }

    fn info_for(symtab: usize, count: u64) -> Info {
        Info {
            strtab: 0,
            // Zero, so the strings run to wherever the symbols start. A stated size would
            // have to be maintained by every test that adds a name.
            strsz: 0,
            symtab: symtab as u64,
            symtabsz: count * SYMBOL_SIZE as u64,
            syment: SYMBOL_SIZE as u64,
            ..Info::default()
        }
    }

    fn packed(id: u16, offset: u32) -> u64 {
        (u64::from(id) << 48) | u64::from(offset)
    }

    #[test]
    fn symbols_are_read_at_the_stated_entry_size() {
        let (mut bytes, offsets) = segment(&["memcpy"]);
        let symtab = bytes.len();
        push_symbol(&mut bytes, offsets[0], 3);

        let read = symbols(&bytes, &info_for(symtab, 1)).expect("symbols");
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].section, 3);
        assert_eq!(read[0].binding(), 1, "global");
        assert_eq!(read[0].kind(), 2, "function");
        assert!(!read[0].is_import(), "a defined symbol is not an import");
    }

    #[test]
    fn a_stated_size_larger_than_the_segment_does_not_allocate_by_it() {
        // The size is a claim. Believing one that runs past the end turns a truncated file
        // into an allocation sized by whatever the field happened to say.
        let (mut bytes, offsets) = segment(&["memcpy"]);
        let symtab = bytes.len();
        push_symbol(&mut bytes, offsets[0], 0);

        let mut info = info_for(symtab, 1);
        info.symtabsz = 1 << 40;
        assert_eq!(
            symbols(&bytes, &info).expect("symbols").len(),
            1,
            "bounded by what is there"
        );
    }

    #[test]
    fn a_zero_entry_size_is_refused_rather_than_looped_on() {
        let (mut bytes, offsets) = segment(&["memcpy"]);
        let symtab = bytes.len();
        push_symbol(&mut bytes, offsets[0], 0);

        let mut info = info_for(symtab, 1);
        info.syment = 0;
        assert!(matches!(
            symbols(&bytes, &info),
            Err(DynamicError::MalformedTable(_))
        ));
    }

    #[test]
    fn a_library_is_found_by_its_id_and_not_by_its_position() {
        // Real material settled this. A vendor module lists its libraries as ids 1, 2, 3, 0,
        // with **libkernel last and numbered zero**. Indexing by position attributes its
        // ninety-six kernel imports to whichever library is listed first, and does it
        // silently, because the answer is a real library name.
        let (mut bytes, offsets) = segment(&["libSceFios2", "libkernel", "wzvqT4UqKX8#A#A"]);
        let symtab = bytes.len();
        push_symbol(&mut bytes, offsets[2], 0);

        let mut info = info_for(symtab, 1);
        info.import_libs = vec![packed(1, offsets[0]), packed(0, offsets[1])];
        info.needed_modules = vec![packed(0, offsets[1])];

        let read = imports(&bytes, &info).expect("imports");
        assert_eq!(read.len(), 1);
        assert_eq!(
            read[0].library,
            Some("libkernel"),
            "id zero is the second entry, not the first"
        );
        assert_eq!(
            read[0].nid,
            selfish_nid::Nid::of("sceKernelLoadStartModule")
        );
    }

    #[test]
    fn a_defined_symbol_is_not_reported_as_an_import() {
        // libc.prx carries 2,676 symbols and imports 109 of them. A reader that skips the
        // section-index test reports a library as importing everything it provides.
        let (mut bytes, offsets) = segment(&["wzvqT4UqKX8#A#A"]);
        let symtab = bytes.len();
        push_symbol(&mut bytes, offsets[0], 1);

        assert!(
            imports(&bytes, &info_for(symtab, 1))
                .expect("imports")
                .is_empty()
        );
    }

    #[test]
    fn an_undefined_symbol_without_ids_is_skipped_and_not_an_error() {
        // Modules hold plain undefined symbols alongside vendor imports. Failing on the
        // first one stops the walk partway and reports a short, plausible list.
        let (mut bytes, offsets) = segment(&["memcpy", "wzvqT4UqKX8#A#A"]);
        let symtab = bytes.len();
        push_symbol(&mut bytes, offsets[0], 0);
        push_symbol(&mut bytes, offsets[1], 0);

        let read = imports(&bytes, &info_for(symtab, 2)).expect("imports");
        assert_eq!(read.len(), 1, "the encoded one, and no error for the other");
    }

    #[test]
    fn an_unnamed_library_leaves_a_hole_rather_than_dropping_the_import() {
        // A module whose library table does not list an id still imports the symbol, and a
        // loader has to see it in order to say what it could not resolve.
        let (mut bytes, offsets) = segment(&["wzvqT4UqKX8#B#B"]);
        let symtab = bytes.len();
        push_symbol(&mut bytes, offsets[0], 0);

        let read = imports(&bytes, &info_for(symtab, 1)).expect("imports");
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].library, None);
        assert_eq!(read[0].module, None);
    }
    #[test]
    fn an_ordinary_elf_belongs_to_neither_convention() {
        // `DT_STRTAB` is 5, and so is the current convention's string-table tag. Deciding on
        // that alone reported every shared object on the system as a vendor module.
        assert_eq!(
            Tags::detect(&[(standard::STRTAB, 0x100), (standard::NEEDED, 1)]),
            None
        );
    }

    #[test]
    fn each_convention_is_recognised_by_something_only_it_can_carry() {
        assert_eq!(
            Tags::detect(&[(vendor::STRTAB, 0)]),
            Some(Table::Legacy),
            "the legacy string-table tag is in the vendor range and unambiguous"
        );
        assert_eq!(
            Tags::detect(&[(standard::STRTAB, 0), (vendor::IMPORT_LIB_CURRENT, 0)]),
            Some(Table::Current),
            "the current convention is known by its identity tags, which are vendor-range"
        );
    }
}
