//! The vendor dynamic table: string table, symbol table, relocations and library lists.
//!
//! [`Table::Orbis`] gives every table a vendor tag in the `0x6100_00xx` range and its values
//! are offsets into `PT_SCE_DYNLIBDATA`. [`Table::Prospero`] uses the standard ELF numbers
//! for the standard tables and vendor tags only for the vendor's own. Reading one with the
//! other's tags yields plausible but wrong offsets.
//!
//! An encoded symbol name's library id indexes the vendor import-library table, not
//! `DT_NEEDED`; the two lists differ in length and content.

use core::fmt;

use selfish_bytes::read_le;
use selfish_nid::Nid;

use crate::reloc::Rela;

/// Standard ELF dynamic tags, used verbatim by [`Table::Prospero`].
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
    pub const FINGERPRINT: u64 = 0x6100_0007;
    /// The module's own filename, as an offset into the string table.
    ///
    /// Required for a shared library; without it a loader refuses the file, counting one
    /// module-info tag and zero of these:
    ///
    /// ```text
    /// [rtld] ERROR preprocess_dt_entries:9600: C: orig fn 0  mod info 1
    /// ```
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

    /// This module's own name and version, in the Orbis convention.
    ///
    /// The module and library tables have vendor tags in both conventions, at different
    /// numbers: `0x0D`-`0x19` for Orbis and `0x43`-`0x49` for Prospero.
    pub const MODULE_INFO: u64 = 0x6100_000D;
    /// A module this one needs. Orbis convention.
    pub const NEEDED_MODULE_ORBIS: u64 = 0x6100_000F;
    /// Module attributes. Orbis convention.
    pub const MODULE_ATTR_ORBIS: u64 = 0x6100_0011;
    /// A library this module exports. Orbis convention.
    pub const EXPORT_LIB_ORBIS: u64 = 0x6100_0013;
    /// The library table an import's library id indexes. Orbis convention.
    pub const IMPORT_LIB_ORBIS: u64 = 0x6100_0015;
    /// Attributes of an exported library.
    pub const EXPORT_LIB_ATTR: u64 = 0x6100_0017;
    /// Attributes of an imported library.
    pub const IMPORT_LIB_ATTR: u64 = 0x6100_0019;

    /// This module's own name and version, in the prospero convention.
    pub const MODULE_INFO_PROSPERO: u64 = 0x6100_0043;
    /// A module this one needs, indexed by an import's module id.
    ///
    /// Also known as `SCE_IMPORT_MODULE`.
    pub const NEEDED_MODULE_PROSPERO: u64 = 0x6100_0045;
    /// Module attributes.
    pub const MODULE_ATTR_PROSPERO: u64 = 0x6100_0047;
    /// The library table an import's library id indexes.
    ///
    /// Not `DT_NEEDED`: it holds exactly as many entries as there are distinct library ids.
    pub const IMPORT_LIB_PROSPERO: u64 = 0x6100_0049;
    /// A library this module exports.
    ///
    /// Unconfirmed: no Prospero-convention module examined carries this tag.
    pub const EXPORT_LIB_PROSPERO: u64 = 0x6100_004D;
}

/// The relocation form every module uses: `Elf64_Rela`.
pub const RELA_FORM: u64 = 7;

/// Size of one relocation entry, and of one symbol entry.
pub const ENTRY_SIZE: u64 = 0x18;

/// Which tag convention a module uses.
///
/// No `Default`, like [`selfish_abi::Generation`]: the caller chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Table {
    /// Vendor tags for everything; the Orbis-generation convention every loader accepts.
    Orbis,
    /// Standard tags for the standard tables and vendor tags for the vendor's own; the
    /// Prospero-generation convention, with numbers read from real modules.
    Prospero,
}

/// The tag numbers for one convention, resolved together so a convention is never half
/// applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    missing_docs,
    reason = "each field is the tag its name says, per convention"
)]
pub struct Tags {
    pub strtab: u64,
    pub strsz: u64,
    pub symtab: u64,
    /// The same vendor tag in both conventions.
    pub symtabsz: u64,
    pub syment: u64,
    pub hash: u64,
    /// The same vendor tag in both conventions, as `symtabsz`.
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
            Table::Orbis => Self {
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
                needed_module: vendor::NEEDED_MODULE_ORBIS,
                module_attr: vendor::MODULE_ATTR_ORBIS,
                import_lib: vendor::IMPORT_LIB_ORBIS,
                export_lib: vendor::EXPORT_LIB_ORBIS,
            },
            Table::Prospero => Self {
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
                module_info: vendor::MODULE_INFO_PROSPERO,
                needed_module: vendor::NEEDED_MODULE_PROSPERO,
                module_attr: vendor::MODULE_ATTR_PROSPERO,
                import_lib: vendor::IMPORT_LIB_PROSPERO,
                export_lib: vendor::EXPORT_LIB_PROSPERO,
            },
        }
    }

    /// Which convention a module is using, from the tags it actually carries.
    ///
    /// Orbis is recognised by its vendor string-table tag. Prospero is recognised by its
    /// module-info, needed-module or import-library tag, since its string-table tag is plain
    /// `DT_STRTAB`, which every ordinary ELF carries. `None` for an ordinary ELF.
    #[must_use]
    pub fn detect(entries: &[(u64, u64)]) -> Option<Table> {
        let prospero = Self::of(Table::Prospero);
        for (tag, _) in entries {
            if *tag == vendor::STRTAB {
                return Some(Table::Orbis);
            }
            if *tag == prospero.module_info
                || *tag == prospero.needed_module
                || *tag == prospero.import_lib
            {
                return Some(Table::Prospero);
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
    /// An import's library id indexes this, not `needed`.
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
        let tags = Tags::of(table.unwrap_or(Table::Orbis));
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
    /// `symtabsz` where present, otherwise `None`; a count is never inferred from a table's
    /// extent.
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
    /// Section index. Zero means undefined, which for these modules means imported.
    pub section: u16,
    /// Address, or zero for an import.
    pub value: u64,
    /// Size, where stated.
    pub size: u64,
}

impl Symbol {
    /// Whether this symbol is defined elsewhere, and therefore imported.
    ///
    /// Section index zero.
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
/// Offsets in the dynamic table are relative to the segment, not to the file.
///
/// Without `symtabsz` this walks to the end of the segment, decoding anything after the
/// symbols as more of them, where [`Info::symbol_count`] answers `None`. A caller that needs
/// the exact count asks `symbol_count` first (D096).
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

    // Bounded by the smaller of the stated size and what is there, so a bad size field
    // cannot size the allocation.
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
        let bad = || DynamicError::TableOutOfRange;
        out.push(Symbol {
            name_offset: read_le(raw, 0).ok_or_else(bad)?,
            info: raw.get(4).copied().unwrap_or(0),
            other: raw.get(5).copied().unwrap_or(0),
            section: read_le(raw, 6).ok_or_else(bad)?,
            value: read_le(raw, 8).ok_or_else(bad)?,
            size: read_le(raw, 16).ok_or_else(bad)?,
        });
        at = at.saturating_add(entry);
    }
    Ok(out)
}

/// An import, resolved as far as the module itself can resolve it.
///
/// The names are what the importing module claims; a loader still has to find them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Import<'a> {
    /// Position in the dynamic symbol table.
    ///
    /// A relocation names its symbol only by this index, so it joins the two tables.
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
/// Empty rather than an error when the table says nothing; a module with no strings is not
/// malformed.
#[must_use]
pub fn strings<'a>(segment: &'a [u8], info: &Info) -> &'a [u8] {
    let base = usize::try_from(info.strtab).unwrap_or(0);
    let size = usize::try_from(info.strsz).unwrap_or(0);
    let rest = segment.get(base..).unwrap_or_default();
    match rest.get(..size) {
        // A stated size larger than what is there falls back to the rest of the segment.
        Some(exact) if size > 0 => exact,
        _ => rest,
    }
}

/// Every symbol this module imports, with its library and module named where they can be.
///
/// Undefined symbols only; a module's own definitions share the table.
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
        // A name without ids is a plain undefined symbol, not a vendor import; modules
        // hold both.
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
/// By id, not by position: the ids are neither dense nor ordered.
fn named<'a>(strings: &'a [u8], table: &[u64], id: u16) -> Option<&'a str> {
    let offset = table.iter().find_map(|packed| {
        let (entry_id, offset) = split_table_entry(*packed);
        (entry_id == id).then_some(offset)
    })?;
    string_at(strings, offset).ok()
}

/// The name of a dynamic tag, where this crate has one.
///
/// Kept beside the constants so printers do not carry their own table. `None` for an
/// unnamed tag, as [`crate::reloc::kind::name`].
///
/// Only the vendor tags and `DT_NEEDED` are named; the other standard numbers double as
/// Prospero-convention table tags.
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
        vendor::MODULE_INFO | vendor::MODULE_INFO_PROSPERO => "DT_SCE_MODULE_INFO",
        vendor::NEEDED_MODULE_ORBIS | vendor::NEEDED_MODULE_PROSPERO => "DT_SCE_NEEDED_MODULE",
        vendor::MODULE_ATTR_ORBIS | vendor::MODULE_ATTR_PROSPERO => "DT_SCE_MODULE_ATTR",
        vendor::EXPORT_LIB_ORBIS => "DT_SCE_EXPORT_LIB",
        vendor::IMPORT_LIB_ORBIS | vendor::IMPORT_LIB_PROSPERO => "DT_SCE_IMPORT_LIB",
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
/// A named pair rather than a tuple, because the two are applied differently.
#[derive(Debug, Clone, Default)]
pub struct Relocations {
    /// `DT_RELA` - data relocations.
    pub data: Vec<Rela>,
    /// `DT_JMPREL` - the procedure linkage table, one slot per imported function.
    pub plt: Vec<Rela>,
}

/// Read both relocation tables out of a vendor segment.
///
/// Offsets are relative to the segment. A range that runs past the end yields an empty
/// table rather than an error.
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

    /// The conventions number the tables differently and share only `symtabsz` and `hashsz`.
    #[test]
    fn the_two_conventions_disagree_where_it_matters_and_agree_where_it_does_not() {
        let orbis = Tags::of(Table::Orbis);
        let prospero = Tags::of(Table::Prospero);

        // The standard tables get different numbers.
        assert_ne!(orbis.strtab, prospero.strtab);
        assert_ne!(orbis.symtab, prospero.symtab);
        assert_ne!(orbis.rela, prospero.rela);

        // The vendor's own tables get vendor tags in both, at different numbers.
        assert_ne!(orbis.import_lib, prospero.import_lib);
        assert_ne!(orbis.module_info, prospero.module_info);
        assert_ne!(orbis.needed_module, prospero.needed_module);

        assert_eq!(orbis.symtabsz, prospero.symtabsz);
        assert_eq!(orbis.hashsz, prospero.hashsz);
    }

    /// A string-table tag alone identifies Orbis, and the standard one identifies nothing.
    #[test]
    fn the_string_table_alone_identifies_only_the_orbis_convention() {
        assert_eq!(
            Tags::detect(&[(vendor::STRTAB, 0x100)]),
            Some(Table::Orbis),
            "the orbis number is in the vendor range and means only one thing"
        );
        assert_eq!(
            Tags::detect(&[(standard::STRTAB, 0x100)]),
            None,
            "the standard number means nothing on its own"
        );
        assert_eq!(Tags::detect(&[(standard::NEEDED, 1)]), None);
    }

    /// A Prospero-convention table is read with standard tags, not vendor ones.
    #[test]
    fn a_prospero_convention_table_is_not_read_with_vendor_tags() {
        let entries = [
            (standard::STRTAB, 0x1000),
            (standard::SYMTAB, 0x2000),
            (standard::RELA, 0x3000),
            (vendor::IMPORT_LIB_PROSPERO, 0x0001_0000_0000_0020),
        ];
        let info = Info::from_entries(&entries);
        assert_eq!(info.table, Some(Table::Prospero));
        assert_eq!(info.strtab, 0x1000);
        assert_eq!(info.symtab, 0x2000);
        assert_eq!(info.rela, 0x3000);
        assert_eq!(info.import_libs.len(), 1);
    }

    /// An Orbis-convention table fills the same fields from vendor tags.
    #[test]
    fn an_orbis_table_reads_the_same_fields_from_different_numbers() {
        let entries = [
            (vendor::STRTAB, 0x1000),
            (vendor::SYMTAB, 0x2000),
            (vendor::RELA, 0x3000),
        ];
        let info = Info::from_entries(&entries);
        assert_eq!(info.table, Some(Table::Orbis));
        assert_eq!(info.strtab, 0x1000);
        assert_eq!(info.symtab, 0x2000);
        assert_eq!(info.rela, 0x3000);
    }

    /// The vendor import-library list is kept apart from `DT_NEEDED`.
    #[test]
    fn the_import_library_table_is_kept_apart_from_needed() {
        let entries = [
            (vendor::STRTAB, 0x1000),
            (standard::NEEDED, 0x10),
            (standard::NEEDED, 0x20),
            (vendor::IMPORT_LIB_ORBIS, 0x0002_0000_0000_0030),
        ];
        let info = Info::from_entries(&entries);
        assert_eq!(info.needed.len(), 2, "DT_NEEDED entries");
        assert_eq!(info.import_libs.len(), 1, "vendor import libraries");
        assert_ne!(
            info.needed.len(),
            info.import_libs.len(),
            "the two lists are different"
        );
    }

    /// A packed entry splits into its top-16-bit id and low-32-bit name offset.
    #[test]
    fn a_packed_table_entry_splits_into_an_id_and_a_name_offset() {
        let (id, offset) = split_table_entry(0x0007_0001_0000_1234);
        assert_eq!(id, 7);
        assert_eq!(offset, 0x1234);
    }

    /// `symbol_count` comes from `symtabsz` or is `None`.
    #[test]
    fn the_symbol_count_comes_from_a_stated_size_or_nowhere() {
        let stated = Info {
            syment: 0x18,
            symtabsz: 0x18 * 5,
            ..Info::default()
        };
        assert_eq!(stated.symbol_count(), Some(5));

        let unstated = Info {
            syment: 0x18,
            ..Info::default()
        };
        assert_eq!(unstated.symbol_count(), None);
    }

    /// Without `symtabsz`, `symbols` reads to the segment's end while `symbol_count` is `None`.
    #[test]
    fn without_a_stated_size_the_reader_infers_a_count_the_info_refuses_to() {
        let segment = vec![0_u8; SYMBOL_SIZE * 4];
        let unstated = Info {
            syment: SYMBOL_SIZE as u64,
            symtab: 0,
            ..Info::default()
        };

        assert_eq!(unstated.symbol_count(), None, "the Info will not guess");
        assert_eq!(
            symbols(&segment, &unstated).expect("reads").len(),
            4,
            "the reader does, from the segment's extent",
        );
    }

    /// `string_at` reads terminated strings and refuses out-of-range or unterminated ones.
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

    /// Build a vendor segment's string table; callers append symbols after it.
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
            // Zero, so the strings run to wherever the symbols start.
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

    /// Symbols are read at the stated entry size with binding and type decoded.
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

    /// A stated size larger than the segment is bounded by the segment.
    #[test]
    fn a_stated_size_larger_than_the_segment_does_not_allocate_by_it() {
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

    /// A zero symbol entry size is refused.
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

    /// An import's library is found by id, not by position; libraries may list id zero last.
    #[test]
    fn a_library_is_found_by_its_id_and_not_by_its_position() {
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

    /// A defined symbol is not reported as an import.
    #[test]
    fn a_defined_symbol_is_not_reported_as_an_import() {
        let (mut bytes, offsets) = segment(&["wzvqT4UqKX8#A#A"]);
        let symtab = bytes.len();
        push_symbol(&mut bytes, offsets[0], 1);

        assert!(
            imports(&bytes, &info_for(symtab, 1))
                .expect("imports")
                .is_empty()
        );
    }

    /// A plain undefined symbol without ids is skipped, not an error.
    #[test]
    fn an_undefined_symbol_without_ids_is_skipped_and_not_an_error() {
        let (mut bytes, offsets) = segment(&["memcpy", "wzvqT4UqKX8#A#A"]);
        let symtab = bytes.len();
        push_symbol(&mut bytes, offsets[0], 0);
        push_symbol(&mut bytes, offsets[1], 0);

        let read = imports(&bytes, &info_for(symtab, 2)).expect("imports");
        assert_eq!(read.len(), 1, "the encoded one, and no error for the other");
    }

    /// An import whose library id is unlisted is kept with no library or module name.
    #[test]
    fn an_unnamed_library_leaves_a_hole_rather_than_dropping_the_import() {
        let (mut bytes, offsets) = segment(&["wzvqT4UqKX8#B#B"]);
        let symtab = bytes.len();
        push_symbol(&mut bytes, offsets[0], 0);

        let read = imports(&bytes, &info_for(symtab, 1)).expect("imports");
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].library, None);
        assert_eq!(read[0].module, None);
    }

    /// An ordinary ELF with `DT_STRTAB` belongs to neither convention.
    #[test]
    fn an_ordinary_elf_belongs_to_neither_convention() {
        assert_eq!(
            Tags::detect(&[(standard::STRTAB, 0x100), (standard::NEEDED, 1)]),
            None
        );
    }

    /// Each convention is recognised by a vendor-range tag only it carries.
    #[test]
    fn each_convention_is_recognised_by_something_only_it_can_carry() {
        assert_eq!(
            Tags::detect(&[(vendor::STRTAB, 0)]),
            Some(Table::Orbis),
            "the orbis string-table tag is in the vendor range and unambiguous"
        );
        assert_eq!(
            Tags::detect(&[(standard::STRTAB, 0), (vendor::IMPORT_LIB_PROSPERO, 0)]),
            Some(Table::Prospero),
            "the prospero convention is known by its identity tags, which are vendor-range"
        );
    }
}
