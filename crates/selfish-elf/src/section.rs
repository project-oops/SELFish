//! Section headers, and the symbol table a linked object carries.
//!
//! **A finished module has none of this.** Sections are a link-time view; the loader reads
//! program headers and the vendor dynamic table, and a stripped module often has `e_shnum`
//! zero. Reading a real `eboot.bin` through this module correctly finds nothing.
//!
//! It is here for the other side. A builder linking a module has an object that *does* carry
//! sections, and it has to look inside them to decide what to emit - whether the module
//! defines an initialiser at all, for one, which decides whether an initialiser tag belongs
//! in the dynamic table. Guessing that wrong produces a module the loader either never
//! initialises or jumps into at an address that holds nothing.
//!
//! # Why this is separate from `dynamic`
//!
//! Two symbol tables exist and they are not the same table. `.dynsym` is what a loader
//! resolves against and lives in the vendor segment; `.symtab` is the full link-time set and
//! lives in a section. A module can carry the first without the second, and treating either
//! as the other gives an answer for the wrong question.

use zerocopy::{FromBytes, Immutable, KnownLayout, little_endian};

/// Size of one section header.
pub const SECTION_HEADER_SIZE: usize = 64;

/// Size of one symbol table entry, which is the same as in the dynamic table.
pub const SYMBOL_SIZE: usize = 24;

/// Section types, as far as this crate needs them.
pub mod kind {
    /// An unused entry. Index zero is always one of these.
    pub const NULL: u32 = 0;
    /// Bytes belonging to the program.
    pub const PROGBITS: u32 = 1;
    /// A symbol table - the link-time one.
    pub const SYMTAB: u32 = 2;
    /// A string table.
    pub const STRTAB: u32 = 3;
    /// Relocations with addends.
    pub const RELA: u32 = 4;
    /// Space with no bytes in the file.
    pub const NOBITS: u32 = 8;
    /// The dynamic linking symbol table.
    pub const DYNSYM: u32 = 11;
}

/// One section header, exactly as it appears on disk.
#[derive(Debug, Clone, Copy, FromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct SectionHeader {
    /// Offset of the name in the section-name string table.
    pub name_offset: little_endian::U32,
    /// What kind of section this is. See [`kind`].
    pub kind: little_endian::U32,
    /// Flags.
    pub flags: little_endian::U64,
    /// Address at run time, or zero.
    pub addr: little_endian::U64,
    /// Offset in the file.
    pub offset: little_endian::U64,
    /// Size in bytes - **in the file, unless this is `NOBITS`**, where nothing is stored.
    pub size: little_endian::U64,
    /// Meaning depends on the type. For a symbol table, the string table's section index.
    pub link: little_endian::U32,
    /// Meaning depends on the type. For a symbol table, one past the last local symbol.
    pub info: little_endian::U32,
    /// Required alignment.
    pub align: little_endian::U64,
    /// Size of one entry, for sections that hold a table.
    pub entry_size: little_endian::U64,
}

impl SectionHeader {
    /// Whether this section occupies space in the file.
    ///
    /// `NOBITS` sections state a size and store nothing - `.bss` is the usual one. Slicing
    /// the file by `size` for one of these reads whatever follows it.
    #[must_use]
    pub fn occupies_file(&self) -> bool {
        self.kind.get() != kind::NOBITS && self.kind.get() != kind::NULL
    }
}

/// One symbol from a linked object's `.symtab`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbol {
    /// Offset of its name in this table's string table.
    pub name_offset: u32,
    /// Binding and type, packed.
    pub info: u8,
    /// Visibility.
    pub other: u8,
    /// Section index, or zero for undefined.
    pub section: u16,
    /// Address.
    pub value: u64,
    /// Size, where stated.
    pub size: u64,
}

impl Symbol {
    /// Whether this symbol is defined elsewhere.
    #[must_use]
    pub const fn is_undefined(&self) -> bool {
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

/// The section table of an object, with everything needed to look inside it.
#[derive(Debug, Clone)]
pub struct Sections<'a> {
    bytes: &'a [u8],
    headers: Vec<SectionHeader>,
    /// Where the section-name table starts.
    ///
    /// An `Option` rather than a zero sentinel. Zero is a legitimate file offset, and a
    /// sentinel that collides with a real value is how a present table reads as absent -
    /// which is what the first version of this did, and the test below is why it was noticed.
    names_at: Option<usize>,
}

impl<'a> Sections<'a> {
    /// Read the section table.
    ///
    /// `Ok(None)` when the file has no sections, which is the normal state of a finished
    /// module rather than a failure. An `Err` means there is a table and it is unreadable.
    ///
    /// # Errors
    ///
    /// If the table runs past the end of the file, or the stated entry size is not 64.
    pub fn parse(
        bytes: &'a [u8],
        offset: u64,
        entry_size: u16,
        count: u16,
        names_index: u16,
    ) -> Result<Option<Self>, SectionError> {
        if count == 0 || offset == 0 {
            return Ok(None);
        }
        if usize::from(entry_size) != SECTION_HEADER_SIZE {
            return Err(SectionError::UnexpectedEntrySize(usize::from(entry_size)));
        }
        let base = usize::try_from(offset).map_err(|_| SectionError::OutOfRange)?;

        let mut headers = Vec::with_capacity(usize::from(count));
        for index in 0..usize::from(count) {
            let at = index
                .checked_mul(SECTION_HEADER_SIZE)
                .and_then(|at| at.checked_add(base))
                .ok_or(SectionError::OutOfRange)?;
            let end = at
                .checked_add(SECTION_HEADER_SIZE)
                .ok_or(SectionError::OutOfRange)?;
            let raw = bytes.get(at..end).ok_or(SectionError::OutOfRange)?;
            headers
                .push(SectionHeader::read_from_bytes(raw).map_err(|_| SectionError::OutOfRange)?);
        }

        // The name table is itself a section, named by index in the file header. An index
        // past the end means names are unavailable, not that the file is unreadable.
        let names_at = headers
            .get(usize::from(names_index))
            .filter(|header| header.occupies_file())
            .and_then(|header| usize::try_from(header.offset.get()).ok());

        Ok(Some(Self {
            bytes,
            headers,
            names_at,
        }))
    }

    /// Every section header.
    #[must_use]
    pub fn headers(&self) -> &[SectionHeader] {
        &self.headers
    }

    /// The name of a section, if the name table is present and readable.
    #[must_use]
    pub fn name(&self, header: &SectionHeader) -> Option<&'a str> {
        let at = self
            .names_at?
            .checked_add(usize::try_from(header.name_offset.get()).ok()?)?;
        let rest = self.bytes.get(at..)?;
        let end = rest
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(rest.len());
        core::str::from_utf8(rest.get(..end)?).ok()
    }

    /// A section by name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&SectionHeader> {
        self.headers
            .iter()
            .find(|header| self.name(header) == Some(name))
    }

    /// A section's bytes.
    #[must_use]
    pub fn contents(&self, header: &SectionHeader) -> Option<&'a [u8]> {
        if !header.occupies_file() {
            return None;
        }
        let at = usize::try_from(header.offset.get()).ok()?;
        let size = usize::try_from(header.size.get()).ok()?;
        self.bytes.get(at..at.checked_add(size)?)
    }

    /// The link-time symbol table, if there is one.
    ///
    /// Returns the symbols and the string table their names live in. The string table is
    /// found through the symbol section's `link` field rather than by the name `.strtab`,
    /// because an object can carry more than one string table and the name of the right one
    /// is not guaranteed.
    #[must_use]
    pub fn symbols(&self) -> Option<(Vec<Symbol>, &'a [u8])> {
        self.symbols_of(kind::SYMTAB)
    }

    /// The dynamic symbol table, `.dynsym`, with its string table.
    ///
    /// A different table from `.symtab`: this is the one a loader resolves against, and the
    /// only symbol table a stripped shared object keeps. Reading a payload's imports - its
    /// undefined dynamic symbols - needs this, because [`Sections::symbols`] reads `.symtab`,
    /// which a stripped object does not carry.
    #[must_use]
    pub fn dynamic_symbols(&self) -> Option<(Vec<Symbol>, &'a [u8])> {
        self.symbols_of(kind::DYNSYM)
    }

    /// Reads a symbol table of the given section kind, with its linked string table.
    fn symbols_of(&self, table_kind: u32) -> Option<(Vec<Symbol>, &'a [u8])> {
        let table = self
            .headers
            .iter()
            .find(|header| header.kind.get() == table_kind)?;
        let strings = self
            .headers
            .get(usize::try_from(table.link.get()).ok()?)
            .and_then(|header| self.contents(header))
            .unwrap_or_default();

        let raw = self.contents(table)?;
        let entry = usize::try_from(table.entry_size.get()).unwrap_or(SYMBOL_SIZE);
        if entry == 0 {
            return None;
        }

        let mut out = Vec::with_capacity(raw.len().checked_div(entry).unwrap_or(0));
        let mut at = 0_usize;
        while at.saturating_add(SYMBOL_SIZE) <= raw.len() {
            let chunk = raw.get(at..at.saturating_add(SYMBOL_SIZE))?;
            out.push(Symbol {
                name_offset: read_u32(chunk, 0)?,
                info: chunk.get(4).copied()?,
                other: chunk.get(5).copied()?,
                section: read_u16(chunk, 6)?,
                value: read_u64(chunk, 8)?,
                size: read_u64(chunk, 16)?,
            });
            at = at.saturating_add(entry);
        }
        Some((out, strings))
    }

    /// Whether the object defines a symbol of this name.
    ///
    /// **Defines**, not mentions. An object references every symbol it imports, and an
    /// existence test that counts those reports a module as defining an initialiser it in
    /// fact expects somebody else to provide.
    #[must_use]
    pub fn defines(&self, name: &str) -> bool {
        let Some((symbols, strings)) = self.symbols() else {
            return false;
        };
        symbols.iter().any(|symbol| {
            !symbol.is_undefined() && string_at(strings, symbol.name_offset) == Some(name)
        })
    }
}

/// A name from a string table.
#[must_use]
pub fn string_at(strings: &[u8], offset: u32) -> Option<&str> {
    let rest = strings.get(usize::try_from(offset).ok()?..)?;
    let end = rest
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(rest.len());
    core::str::from_utf8(rest.get(..end)?).ok()
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    let mut out = [0_u8; 2];
    out.copy_from_slice(bytes.get(at..at.checked_add(2)?)?);
    Some(u16::from_le_bytes(out))
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let mut out = [0_u8; 4];
    out.copy_from_slice(bytes.get(at..at.checked_add(4)?)?);
    Some(u32::from_le_bytes(out))
}

fn read_u64(bytes: &[u8], at: usize) -> Option<u64> {
    let mut out = [0_u8; 8];
    out.copy_from_slice(bytes.get(at..at.checked_add(8)?)?);
    Some(u64::from_le_bytes(out))
}

/// What can go wrong reading a section table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SectionError {
    /// The table runs past the end of the file.
    OutOfRange,
    /// The file states a section header size other than 64.
    UnexpectedEntrySize(usize),
}

impl core::fmt::Display for SectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OutOfRange => write!(f, "the section table runs past the end of the file"),
            Self::UnexpectedEntrySize(size) => {
                write!(f, "section headers are {size} bytes rather than 64")
            }
        }
    }
}

impl std::error::Error for SectionError {}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{SECTION_HEADER_SIZE, SectionError, Sections, kind};

    /// Build an object with a name table, a string table and a symbol table.
    fn object(symbols: &[(&str, u16)]) -> Vec<u8> {
        let mut names = vec![0_u8];
        let mut offsets = Vec::new();
        for name in [".shstrtab", ".strtab", ".symtab", ".text"] {
            offsets.push(u32::try_from(names.len()).unwrap());
            names.extend_from_slice(name.as_bytes());
            names.push(0);
        }

        let mut strings = vec![0_u8];
        let mut symbol_names = Vec::new();
        for (name, _) in symbols {
            symbol_names.push(u32::try_from(strings.len()).unwrap());
            strings.extend_from_slice(name.as_bytes());
            strings.push(0);
        }

        let mut table = Vec::new();
        for (index, (_, section)) in symbols.iter().enumerate() {
            table.extend_from_slice(&symbol_names[index].to_le_bytes());
            table.push(0x12);
            table.push(0);
            table.extend_from_slice(&section.to_le_bytes());
            table.extend_from_slice(&0_u64.to_le_bytes());
            table.extend_from_slice(&0_u64.to_le_bytes());
        }

        // Contents first, then the section table, so every offset is known when it is written.
        let mut bytes = Vec::new();
        let names_at = bytes.len();
        bytes.extend_from_slice(&names);
        let strings_at = bytes.len();
        bytes.extend_from_slice(&strings);
        let table_at = bytes.len();
        bytes.extend_from_slice(&table);
        let sections_at = bytes.len();

        let mut header = |name: u32, kind: u32, offset: usize, size: usize, link: u32| {
            bytes.extend_from_slice(&name.to_le_bytes());
            bytes.extend_from_slice(&kind.to_le_bytes());
            bytes.extend_from_slice(&0_u64.to_le_bytes()); // flags
            bytes.extend_from_slice(&0_u64.to_le_bytes()); // addr
            bytes.extend_from_slice(&(offset as u64).to_le_bytes());
            bytes.extend_from_slice(&(size as u64).to_le_bytes());
            bytes.extend_from_slice(&link.to_le_bytes());
            bytes.extend_from_slice(&0_u32.to_le_bytes()); // info
            bytes.extend_from_slice(&0_u64.to_le_bytes()); // align
            bytes.extend_from_slice(&24_u64.to_le_bytes()); // entry size
        };
        header(offsets[0], kind::STRTAB, names_at, names.len(), 0);
        header(offsets[1], kind::STRTAB, strings_at, strings.len(), 0);
        header(offsets[2], kind::SYMTAB, table_at, table.len(), 1);

        // Section index 0 is the name table here, which is what `names_index` points at -
        // deliberately, since it puts the name table at file offset zero and so covers the
        // sentinel case below.
        let _ = sections_at;
        bytes
    }

    fn sections(bytes: &[u8], count: u16) -> Sections<'_> {
        let table_at = bytes.len() - usize::from(count) * SECTION_HEADER_SIZE;
        Sections::parse(bytes, table_at as u64, 64, count, 0)
            .expect("a table")
            .expect("present")
    }

    #[test]
    fn a_file_with_no_sections_is_not_an_error() {
        // A finished module usually has none, and reporting that as a failure would make
        // every real `eboot.bin` unreadable through this module.
        assert!(
            Sections::parse(&[], 0, 64, 0, 0)
                .expect("no error")
                .is_none()
        );
        assert!(
            Sections::parse(&[0; 64], 0, 64, 3, 0)
                .expect("no error")
                .is_none(),
            "an offset of zero also means absent"
        );
    }

    #[test]
    fn an_unexpected_header_size_is_refused_rather_than_read_at_the_wrong_stride() {
        assert_eq!(
            Sections::parse(&[0; 256], 64, 40, 2, 0).unwrap_err(),
            SectionError::UnexpectedEntrySize(40)
        );
    }

    #[test]
    fn sections_are_found_by_name_even_when_the_name_table_is_at_offset_zero() {
        // Zero is a legitimate file offset. The first version used it as the "no name table"
        // sentinel and reported every section as unnamed here.
        let bytes = object(&[("main", 1)]);
        let table = sections(&bytes, 3);
        assert!(table.find(".symtab").is_some());
        assert!(table.find(".nonesuch").is_none());
    }

    #[test]
    fn the_string_table_is_found_through_link_rather_than_by_name() {
        // An object can carry more than one string table, and the name of the right one is
        // not guaranteed. `link` is what says which.
        let bytes = object(&[("_init", 1)]);
        let table = sections(&bytes, 3);
        let (symbols, strings) = table.symbols().expect("a symbol table");
        assert_eq!(symbols.len(), 1);
        assert_eq!(
            super::string_at(strings, symbols[0].name_offset),
            Some("_init")
        );
    }

    #[test]
    fn defines_means_defined_and_not_merely_mentioned() {
        // An object references every symbol it imports. Counting those reports a module as
        // defining an initialiser it in fact expects somebody else to provide - and the
        // builder then emits a tag pointing at nothing.
        let bytes = object(&[("_init", 1), ("memcpy", 0)]);
        let table = sections(&bytes, 3);
        assert!(table.defines("_init"), "defined in section one");
        assert!(!table.defines("memcpy"), "undefined, so imported");
        assert!(!table.defines("nonesuch"));
    }

    #[test]
    fn a_nobits_section_yields_no_contents() {
        // `.bss` states a size and stores nothing. Slicing the file by that size reads
        // whatever follows it, which is a real section's bytes under another name.
        let bytes = object(&[("main", 1)]);
        let table = sections(&bytes, 3);
        let mut header = table.headers()[1];
        header.kind = kind::NOBITS.into();
        assert!(table.contents(&header).is_none());
    }
}
