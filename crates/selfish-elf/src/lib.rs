//! The executable format as the platform spells it.
//!
//! A vendor executable is an ELF64 with a small number of differences, each of which a
//! loader checks before it looks at a single symbol - so getting any of them wrong produces
//! a rejection that says nothing about the rest of the file.
//!
//! # The differences that matter
//!
//! - **`EI_OSABI` is FreeBSD.** `lld` sets it; GNU `ld` does not, and a module linked with
//!   the latter is refused on that byte alone.
//! - **`EI_ABIVERSION` carries the generation.** Read before any guest instruction runs, so
//!   it cannot be negotiated: it is decided when the file is built. See [`selfish_abi`].
//! - **`e_type` is outside the standard range**, and the two values it takes mean
//!   *executable* and *shared library*. A loader that respects the difference runs a
//!   library's initialisers and then looks elsewhere for an entry point, which is a silent
//!   no-op rather than an error.
//! - **The dynamic table a loader reads is the vendor's**, carried in its own segment. The
//!   standard one is present and ignored.
//!
//! # Parsing without `unsafe`
//!
//! `zerocopy` validates size and alignment before a reference exists, so hostile bytes are a
//! parse failure rather than a fault. This crate contains no `unsafe` and the workspace
//! forbids it.

#![forbid(unsafe_code)]

pub mod dynamic;
pub mod dynlib;
pub mod identity;
pub mod layout;
pub mod reloc;
pub mod section;

use core::fmt;

use selfish_abi::Generation;
use zerocopy::{FromBytes, Immutable, KnownLayout, little_endian};

/// The four bytes every ELF begins with.
pub const MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

/// Size of the ELF64 file header.
pub const HEADER_SIZE: usize = 64;

/// Size of one program header entry.
pub const PROGRAM_HEADER_SIZE: usize = 56;

/// `EI_CLASS` for a 64-bit object.
pub const CLASS64: u8 = 2;

/// `EI_DATA` for little-endian.
pub const DATA_LSB: u8 = 1;

/// `EI_OSABI` the platform requires.
///
/// FreeBSD, because the kernel is FreeBSD-derived. `lld` sets this; GNU `ld` does not, and
/// the resulting module is refused before anything else is examined.
pub const OSABI_FREEBSD: u8 = 9;

/// Index of `EI_OSABI` within `e_ident`.
pub const EI_OSABI: usize = 7;

/// Index of `EI_ABIVERSION` within `e_ident`.
pub const EI_ABIVERSION: usize = 8;

/// Index of `EI_CLASS`.
pub const EI_CLASS: usize = 4;

/// Index of `EI_DATA`.
pub const EI_DATA: usize = 5;

/// x86-64.
pub const MACHINE_X86_64: u16 = 0x3E;

/// What an `e_type` says the file is.
///
/// The two vendor values differ by one bit and mean entirely different things, which is
/// exactly how the wrong one survives: a loader that does not distinguish them runs either
/// quite happily, and only one that does reveals the mistake - as a program that loads,
/// relocates, runs its initialisers and then does nothing at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    /// Fixed-address executable.
    FixedExecutable,
    /// Position-independent executable. What an `eboot` is.
    Executable,
    /// Shared library. What a `.prx` is.
    SharedLibrary,
    /// Anything else, including the standard ELF types.
    Other(u16),
}

impl ObjectType {
    /// `e_type` for a fixed-address executable.
    pub const FIXED_EXECUTABLE: u16 = 0xFE00;
    /// `e_type` for a position-independent executable.
    pub const EXECUTABLE: u16 = 0xFE10;
    /// `e_type` for a shared library.
    pub const SHARED_LIBRARY: u16 = 0xFE18;

    /// Read from the raw value.
    #[must_use]
    pub const fn from_raw(value: u16) -> Self {
        match value {
            Self::FIXED_EXECUTABLE => Self::FixedExecutable,
            Self::EXECUTABLE => Self::Executable,
            Self::SHARED_LIBRARY => Self::SharedLibrary,
            other => Self::Other(other),
        }
    }

    /// The raw value.
    #[must_use]
    pub const fn to_raw(self) -> u16 {
        match self {
            Self::FixedExecutable => Self::FIXED_EXECUTABLE,
            Self::Executable => Self::EXECUTABLE,
            Self::SharedLibrary => Self::SHARED_LIBRARY,
            Self::Other(value) => value,
        }
    }

    /// Whether this is one of the platform's own types rather than a standard ELF one.
    ///
    /// The three a loader names when it refuses a file: `e_type expected 0xFE10 OR 0xFE18 OR
    /// 0xfe00`.
    #[must_use]
    pub const fn is_platform(self) -> bool {
        matches!(
            self,
            Self::FixedExecutable | Self::Executable | Self::SharedLibrary
        )
    }

    /// Whether a loader will look here for a process to start.
    ///
    /// The distinction a whole class of silent failure rests on: a loader that respects it
    /// runs a shared library's initialisers and then looks *elsewhere* for an entry point.
    #[must_use]
    pub const fn is_executable(self) -> bool {
        matches!(self, Self::FixedExecutable | Self::Executable)
    }
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FixedExecutable => write!(f, "fixed-address executable"),
            Self::Executable => write!(f, "executable"),
            Self::SharedLibrary => write!(f, "shared library"),
            Self::Other(v) => write!(f, "e_type {v:#06x}"),
        }
    }
}

/// Program header types, including the vendor's.
pub mod segment {
    /// Loadable segment.
    pub const LOAD: u32 = 0x1;
    /// The standard dynamic table. Present in a vendor module and ignored by its loader.
    pub const DYNAMIC: u32 = 0x2;
    /// Interpreter path.
    pub const INTERP: u32 = 0x3;
    /// Thread-local storage.
    pub const TLS: u32 = 0x7;

    /// The vendor's relocation table.
    pub const SCE_RELA: u32 = 0x6000_0000;
    /// **The segment a loader actually reads**: strings, symbols, hashes and relocations.
    pub const SCE_DYNLIBDATA: u32 = 0x6100_0000;
    /// Process parameters.
    pub const SCE_PROCPARAM: u32 = 0x6100_0001;
    /// Module parameters.
    pub const SCE_MODULE_PARAM: u32 = 0x6100_0002;
    /// Read-only after relocation.
    pub const SCE_RELRO: u32 = 0x6100_0010;
    /// Comment.
    pub const SCE_COMMENT: u32 = 0x6FFF_FF00;
    /// Version.
    pub const SCE_VERSION: u32 = 0x6FFF_FF01;

    /// The OS-specific range, per the ELF specification.
    ///
    /// Everything vendor-defined is in here - but so are the GNU extensions, which are
    /// ordinary and not vendor data at all. Treating the whole range as vendor-specific
    /// misclassifies three perfectly standard segment types.
    pub const OS_SPECIFIC: core::ops::RangeInclusive<u32> = 0x6000_0000..=0x6FFF_FFFF;

    /// GNU extensions living inside the OS-specific range.
    pub const GNU: [u32; 3] = [0x6474_E550, 0x6474_E551, 0x6474_E552];

    /// Whether a segment type is the vendor's rather than a GNU extension.
    #[must_use]
    pub fn is_vendor(p_type: u32) -> bool {
        OS_SPECIFIC.contains(&p_type) && !GNU.contains(&p_type)
    }
}

/// Raw ELF64 file header, exactly as it appears on disk.
#[derive(Debug, Clone, Copy, FromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct RawHeader {
    /// Magic, class, endianness, version, ABI.
    pub ident: [u8; 16],
    /// Object file type. The vendor uses values outside the standard range.
    pub e_type: little_endian::U16,
    /// Target architecture.
    pub machine: little_endian::U16,
    /// ELF version.
    pub version: little_endian::U32,
    /// Entry point.
    pub entry: little_endian::U64,
    /// File offset of the program header table.
    pub phoff: little_endian::U64,
    /// File offset of the section header table.
    pub shoff: little_endian::U64,
    /// Processor-specific flags.
    pub flags: little_endian::U32,
    /// Size of this header.
    pub ehsize: little_endian::U16,
    /// Size of one program header entry.
    pub phentsize: little_endian::U16,
    /// Number of program header entries.
    pub phnum: little_endian::U16,
    /// Size of one section header entry.
    pub shentsize: little_endian::U16,
    /// Number of section header entries.
    pub shnum: little_endian::U16,
    /// Section index of the section-name string table.
    pub shstrndx: little_endian::U16,
}

/// Raw ELF64 program header.
#[derive(Debug, Clone, Copy, FromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct RawProgramHeader {
    /// Segment type.
    pub p_type: little_endian::U32,
    /// Permission flags.
    pub flags: little_endian::U32,
    /// File offset of the contents.
    pub offset: little_endian::U64,
    /// Virtual address it loads at.
    pub vaddr: little_endian::U64,
    /// Physical address. Unused.
    pub paddr: little_endian::U64,
    /// Bytes present in the file.
    pub filesz: little_endian::U64,
    /// Bytes occupied in memory, which may exceed `filesz`.
    pub memsz: little_endian::U64,
    /// Required alignment.
    pub align: little_endian::U64,
}

/// A parsed executable, borrowing its bytes.
#[derive(Debug)]
pub struct Elf<'a> {
    bytes: &'a [u8],
    header: RawHeader,
    program_headers: Vec<RawProgramHeader>,
}

impl<'a> Elf<'a> {
    /// Parse an executable.
    ///
    /// # Errors
    ///
    /// If the magic, class or endianness are wrong, or the program header table does not
    /// fit inside the file.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ElfError> {
        let head = bytes.get(..HEADER_SIZE).ok_or(ElfError::TooShort {
            needed: HEADER_SIZE,
            found: bytes.len(),
        })?;
        let header = RawHeader::read_from_bytes(head).map_err(|_| ElfError::TooShort {
            needed: HEADER_SIZE,
            found: bytes.len(),
        })?;

        let magic = header.ident.get(..4).unwrap_or_default();
        if magic != MAGIC {
            let mut found = [0_u8; 4];
            found.copy_from_slice(magic.get(..4).unwrap_or(&[0; 4]));
            return Err(ElfError::NotAnElf(found));
        }
        if header.ident.get(EI_CLASS) != Some(&CLASS64) {
            return Err(ElfError::NotSixtyFourBit);
        }
        if header.ident.get(EI_DATA) != Some(&DATA_LSB) {
            return Err(ElfError::NotLittleEndian);
        }

        let phnum = usize::from(header.phnum.get());
        let phentsize = usize::from(header.phentsize.get());
        let phoff = usize::try_from(header.phoff.get()).unwrap_or(usize::MAX);
        let mut program_headers = Vec::with_capacity(phnum);
        if phnum > 0 {
            if phentsize != PROGRAM_HEADER_SIZE {
                return Err(ElfError::UnexpectedProgramHeaderSize(phentsize));
            }
            for index in 0..phnum {
                let at = index
                    .checked_mul(phentsize)
                    .and_then(|o| o.checked_add(phoff))
                    .ok_or(ElfError::ProgramHeadersOutOfBounds)?;
                let end = at
                    .checked_add(phentsize)
                    .ok_or(ElfError::ProgramHeadersOutOfBounds)?;
                let raw = bytes
                    .get(at..end)
                    .ok_or(ElfError::ProgramHeadersOutOfBounds)?;
                program_headers.push(
                    RawProgramHeader::read_from_bytes(raw)
                        .map_err(|_| ElfError::ProgramHeadersOutOfBounds)?,
                );
            }
        }

        Ok(Self {
            bytes,
            header,
            program_headers,
        })
    }

    /// The whole file.
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// The raw file header.
    #[must_use]
    pub const fn header(&self) -> &RawHeader {
        &self.header
    }

    /// Program headers, in file order.
    #[must_use]
    pub fn program_headers(&self) -> &[RawProgramHeader] {
        &self.program_headers
    }

    /// What the file says it is.
    #[must_use]
    pub fn object_type(&self) -> ObjectType {
        ObjectType::from_raw(self.header.e_type.get())
    }

    /// Which generation it was built for, from `EI_ABIVERSION`.
    ///
    /// `None` only for a byte that is neither generation's - `1`, `3`, anything else. That is
    /// a narrower answer than it looks, and the narrowness is the point:
    ///
    /// **Zero is the previous generation's own value, so an ordinary ELF is indistinguishable
    /// from a previous-generation module by this byte alone.** Both read as
    /// [`Generation::Previous`]. A caller that needs to tell them apart has to ask something
    /// else - [`Self::has_platform_osabi`] and [`Self::object_type`] between them do it, since
    /// an ordinary object carries neither the platform's `EI_OSABI` nor one of its three
    /// `e_type` values.
    ///
    /// Said explicitly because this comment previously claimed the opposite - that zero here
    /// answered `None` "for reasons that have nothing to do with a console" - which is a
    /// branch that cannot be reached for that reason and would have had a caller trusting a
    /// distinction this cannot make.
    #[must_use]
    pub fn generation(&self) -> Option<Generation> {
        match self.header.ident.get(EI_ABIVERSION) {
            Some(&b) if b == Generation::Current.abi_version() => Some(Generation::Current),
            Some(&b) if b == Generation::Previous.abi_version() => Some(Generation::Previous),
            _ => None,
        }
    }

    /// Whether `EI_OSABI` is the one the platform requires.
    #[must_use]
    pub fn has_platform_osabi(&self) -> bool {
        self.header.ident.get(EI_OSABI) == Some(&OSABI_FREEBSD)
    }

    /// The entry point, unadjusted for a load base.
    #[must_use]
    pub fn entry(&self) -> u64 {
        self.header.entry.get()
    }

    /// The first segment of a given type.
    #[must_use]
    pub fn segment(&self, p_type: u32) -> Option<&RawProgramHeader> {
        self.program_headers
            .iter()
            .find(|p| p.p_type.get() == p_type)
    }

    /// The bytes of a segment, if they are within the file.
    #[must_use]
    pub fn segment_bytes(&self, phdr: &RawProgramHeader) -> Option<&'a [u8]> {
        let start = usize::try_from(phdr.offset.get()).ok()?;
        let len = usize::try_from(phdr.filesz.get()).ok()?;
        self.bytes.get(start..start.checked_add(len)?)
    }

    /// The dynamic table's `(tag, value)` pairs.
    ///
    /// Read from `PT_DYNAMIC`, which a vendor module carries alongside the vendor segment
    /// its tags point into. The pairs are sixteen bytes each and a zero tag ends the table.
    ///
    /// # Errors
    ///
    /// If the segment's contents are not within the file.
    pub fn dynamic_entries(&self) -> Result<Vec<(u64, u64)>, ElfError> {
        let Some(phdr) = self.segment(segment::DYNAMIC) else {
            return Ok(Vec::new());
        };
        let bytes = self
            .segment_bytes(phdr)
            .ok_or(ElfError::ProgramHeadersOutOfBounds)?;
        let mut out = Vec::new();
        let mut at = 0_usize;
        while let Some(entry) = bytes.get(at..at.saturating_add(16)) {
            let mut tag = [0_u8; 8];
            let mut value = [0_u8; 8];
            tag.copy_from_slice(entry.get(..8).unwrap_or(&[0; 8]));
            value.copy_from_slice(entry.get(8..16).unwrap_or(&[0; 8]));
            let tag = u64::from_le_bytes(tag);
            // A zero tag is the terminator, and stopping there matters: the segment is
            // usually padded, and reading past it yields a long tail of (0, 0) pairs that
            // look like entries.
            if tag == 0 {
                break;
            }
            out.push((tag, u64::from_le_bytes(value)));
            at = at.saturating_add(16);
        }
        Ok(out)
    }

    /// The vendor segment the dynamic tags point into.
    #[must_use]
    pub fn vendor_segment(&self) -> Option<&'a [u8]> {
        self.segment(segment::SCE_DYNLIBDATA)
            .and_then(|phdr| self.segment_bytes(phdr))
    }

    /// The segment holding the dynamic tables, with every offset already relative to it.
    ///
    /// **This is what a reader should use rather than [`Self::vendor_segment`]**, because the
    /// two conventions put the tables in different places and measure them from different
    /// origins, and the two halves go together:
    ///
    /// - **Legacy**: a `PT_SCE_DYNLIBDATA` segment that is never mapped, with every table tag
    ///   holding an *offset into it*.
    /// - **Current**: no such segment exists - the tables are in the image, and the tags hold
    ///   *virtual addresses*. The segment is found by which `PT_LOAD` contains the string
    ///   table, which is also how a loader finds it, so it is not the weaker test it looks.
    ///
    /// The returned [`dynamic::Info`] has its table offsets rebased, so everything in
    /// [`dynamic`] reads either convention without knowing which it was handed.
    ///
    /// # How far the current-convention path has been checked
    ///
    /// Against **this crate's own writer**, not against a console. `tests/current.rs` builds a
    /// current-convention module with [`crate::dynlib`], reads it back through here, and
    /// asserts the imports match what the same source produces under the legacy convention.
    /// That proves the two halves agree about the tag numbers, the virtual-address origin and
    /// the rebasing.
    ///
    /// It does **not** prove a console agrees, because every module this repository has been
    /// pointed at is previous-generation. If current-generation material ever turns up, that
    /// test is where to point it.
    ///
    /// # Errors
    ///
    /// If the dynamic table cannot be read. `Ok(None)` when the module carries no vendor
    /// tables at all, which is the correct answer for an ordinary ELF.
    pub fn tables(&self) -> Result<Option<(&'a [u8], dynamic::Info)>, ElfError> {
        let entries = self.dynamic_entries()?;
        let mut info = dynamic::Info::from_entries(&entries);
        let Some(table) = info.table else {
            return Ok(None);
        };

        let (bytes, origin) = match table {
            dynamic::Table::Legacy => match self.segment(segment::SCE_DYNLIBDATA) {
                Some(phdr) => (self.segment_bytes(phdr), 0),
                None => return Ok(None),
            },
            dynamic::Table::Current => {
                let holder = self.program_headers.iter().find(|header| {
                    header.p_type.get() == segment::LOAD
                        && info.strtab >= header.vaddr.get()
                        && info.strtab < header.vaddr.get().saturating_add(header.memsz.get())
                });
                match holder {
                    Some(phdr) => (self.segment_bytes(phdr), phdr.vaddr.get()),
                    None => return Ok(None),
                }
            }
        };
        let Some(bytes) = bytes else {
            return Ok(None);
        };

        // Sizes are sizes and `pltgot` is an address; only the table locations move.
        for offset in [
            &mut info.strtab,
            &mut info.symtab,
            &mut info.hash,
            &mut info.rela,
            &mut info.jmprel,
        ] {
            *offset = offset.saturating_sub(origin);
        }
        Ok(Some((bytes, info)))
    }

    /// Where the program header table ends, which is where a container's copy of the
    /// headers must reach to.
    #[must_use]
    pub fn header_span(&self) -> u64 {
        let phoff = self.header.phoff.get();
        let table = u64::from(self.header.phnum.get())
            .saturating_mul(u64::from(self.header.phentsize.get()));
        let after_table = phoff.saturating_add(table);
        let ehsize = u64::from(self.header.ehsize.get());
        if after_table > ehsize {
            after_table
        } else {
            ehsize
        }
    }

    /// The section table, if this file carries one.
    ///
    /// `Ok(None)` for a finished module, which normally has no sections at all - that is the
    /// expected state rather than a failure. See [`section`].
    ///
    /// # Errors
    ///
    /// If a table is present and unreadable.
    pub fn sections(&self) -> Result<Option<section::Sections<'a>>, section::SectionError> {
        section::Sections::parse(
            self.bytes,
            self.header.shoff.get(),
            self.header.shentsize.get(),
            self.header.shnum.get(),
            self.header.shstrndx.get(),
        )
    }
}

/// Why a file could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfError {
    /// Shorter than a file header.
    TooShort {
        /// Bytes required.
        needed: usize,
        /// Bytes present.
        found: usize,
    },
    /// The first four bytes are not `\x7fELF`.
    ///
    /// Carries what was found, because the commonest wrong answer is a *container*, and
    /// saying which container is far more useful than saying "not an ELF".
    NotAnElf([u8; 4]),
    /// Not a 64-bit object.
    NotSixtyFourBit,
    /// Not little-endian.
    NotLittleEndian,
    /// A program header entry is not the size the format defines.
    UnexpectedProgramHeaderSize(usize),
    /// The program header table runs past the end of the file.
    ProgramHeadersOutOfBounds,
    /// `e_type` is neither what a linker produces nor one of the two the platform accepts.
    ///
    /// Raised by [`identity::stamp`] rather than by parsing: a reader is happy to describe any
    /// object type, but stamping one onto a file this code does not understand would assert
    /// something untrue about it.
    UnexpectedObjectType(u16),
}

impl fmt::Display for ElfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { needed, found } => {
                write!(f, "an ELF header is {needed} bytes, this file is {found}")
            }
            Self::NotAnElf(found) => {
                write!(
                    f,
                    "not an ELF: begins {:02x} {:02x} {:02x} {:02x}",
                    found.first().copied().unwrap_or(0),
                    found.get(1).copied().unwrap_or(0),
                    found.get(2).copied().unwrap_or(0),
                    found.get(3).copied().unwrap_or(0)
                )
            }
            Self::NotSixtyFourBit => write!(f, "not a 64-bit object"),
            Self::NotLittleEndian => write!(f, "not little-endian"),
            Self::UnexpectedProgramHeaderSize(n) => {
                write!(
                    f,
                    "a program header is {PROGRAM_HEADER_SIZE} bytes, this says {n}"
                )
            }
            Self::ProgramHeadersOutOfBounds => {
                write!(f, "the program header table runs past the end of the file")
            }
            Self::UnexpectedObjectType(raw) => {
                write!(f, "e_type {raw:#06x} is not one this can stamp")
            }
        }
    }
}

impl std::error::Error for ElfError {}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "fixture builders read better indexed, and a panic here is the test failing"
)]
mod tests {
    use super::{Elf, ElfError, MAGIC, ObjectType, segment};
    use selfish_abi::Generation;

    /// A minimal well-formed header with one program header.
    fn sample(e_type: u16, abi_version: u8) -> Vec<u8> {
        let mut out = vec![0_u8; super::HEADER_SIZE + super::PROGRAM_HEADER_SIZE];
        out[..4].copy_from_slice(&MAGIC);
        out[super::EI_CLASS] = super::CLASS64;
        out[super::EI_DATA] = super::DATA_LSB;
        out[super::EI_OSABI] = super::OSABI_FREEBSD;
        out[super::EI_ABIVERSION] = abi_version;
        out[16..18].copy_from_slice(&e_type.to_le_bytes());
        out[18..20].copy_from_slice(&super::MACHINE_X86_64.to_le_bytes());
        // phoff = 64, ehsize = 64, phentsize = 56, phnum = 1
        out[32..40].copy_from_slice(&64_u64.to_le_bytes());
        out[52..54].copy_from_slice(&64_u16.to_le_bytes());
        out[54..56].copy_from_slice(&56_u16.to_le_bytes());
        out[56..58].copy_from_slice(&1_u16.to_le_bytes());
        // One PT_LOAD.
        out[64..68].copy_from_slice(&segment::LOAD.to_le_bytes());
        out
    }

    #[test]
    fn a_well_formed_module_parses() {
        let bytes = sample(ObjectType::EXECUTABLE, 2);
        let elf = Elf::parse(&bytes).expect("parses");
        assert_eq!(elf.object_type(), ObjectType::Executable);
        assert_eq!(elf.generation(), Some(Generation::Current));
        assert!(elf.has_platform_osabi());
        assert_eq!(elf.program_headers().len(), 1);
    }

    #[test]
    fn the_two_vendor_types_are_not_interchangeable() {
        // The distinction a silent no-op rests on: a library's initialisers run and then a
        // loader looks elsewhere for an entry point, which is not an error anywhere.
        assert!(ObjectType::from_raw(ObjectType::EXECUTABLE).is_executable());
        assert!(ObjectType::from_raw(ObjectType::FIXED_EXECUTABLE).is_executable());
        assert!(!ObjectType::from_raw(ObjectType::SHARED_LIBRARY).is_executable());
    }

    #[test]
    fn object_type_round_trips_including_values_it_does_not_know() {
        for raw in [0xFE00, 0xFE10, 0xFE18, 0x0002, 0x0003, 0xFFFF] {
            assert_eq!(ObjectType::from_raw(raw).to_raw(), raw);
        }
    }

    #[test]
    fn abi_version_tells_the_generations_apart() {
        for (byte, want) in [
            (2_u8, Some(Generation::Current)),
            (0, Some(Generation::Previous)),
            // A byte matching neither is not a third console.
            (7, None),
        ] {
            let bytes = sample(ObjectType::EXECUTABLE, byte);
            let elf = Elf::parse(&bytes).expect("parses");
            assert_eq!(elf.generation(), want, "EI_ABIVERSION {byte}");
        }
    }

    #[test]
    fn a_container_is_reported_as_what_it_is_rather_than_as_not_an_elf() {
        // The commonest wrong input by far, and "not an ELF" would send somebody looking in
        // entirely the wrong place.
        let container = [0x54_u8, 0x14, 0xF5, 0xEE];
        let mut bytes = vec![0_u8; super::HEADER_SIZE];
        bytes[..4].copy_from_slice(&container);
        assert_eq!(
            Elf::parse(&bytes).unwrap_err(),
            ElfError::NotAnElf(container)
        );
    }

    #[test]
    fn a_truncated_file_is_refused_rather_than_read_past() {
        assert!(matches!(
            Elf::parse(&[0_u8; 8]),
            Err(ElfError::TooShort { .. })
        ));
    }

    #[test]
    fn a_program_header_table_past_the_end_is_refused() {
        let mut bytes = sample(ObjectType::EXECUTABLE, 2);
        // Claim forty headers when only one is present.
        bytes[56..58].copy_from_slice(&40_u16.to_le_bytes());
        assert_eq!(
            Elf::parse(&bytes).unwrap_err(),
            ElfError::ProgramHeadersOutOfBounds
        );
    }

    #[test]
    fn gnu_segments_are_not_mistaken_for_vendor_ones() {
        // Both live in the OS-specific range. Treating the whole range as the vendor's
        // misclassifies three ordinary segment types.
        assert!(segment::is_vendor(segment::SCE_DYNLIBDATA));
        assert!(segment::is_vendor(segment::SCE_RELRO));
        for gnu in segment::GNU {
            assert!(!segment::is_vendor(gnu), "{gnu:#x} is a GNU extension");
        }
        assert!(!segment::is_vendor(segment::LOAD));
    }

    #[test]
    fn header_span_covers_the_program_header_table() {
        let bytes = sample(ObjectType::EXECUTABLE, 2);
        let elf = Elf::parse(&bytes).expect("parses");
        // 64-byte header plus one 56-byte entry at offset 64.
        assert_eq!(elf.header_span(), 120);
    }
}
