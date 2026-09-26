//! What the reader says about a file, and what it refuses.
//!
//! Synthetic bytes, because a linker cannot emit the malformed inputs a reader must refuse.
//! The `Display` text is part of the contract: a refusal carries what it found.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    reason = "a panic in a test is the test failing"
)]

use selfish_abi::Generation;
use selfish_elf::{
    CLASS64, DATA_LSB, EI_ABIVERSION, EI_CLASS, EI_DATA, EI_OSABI, Elf, ElfError, HEADER_SIZE,
    MACHINE_X86_64, MAGIC, OSABI_FREEBSD, ObjectType, PROGRAM_HEADER_SIZE, segment,
};

/// Offsets inside the file header, from the ELF64 specification rather than the parser's
/// constants, so the test is independent of them.
mod at {
    pub(crate) const E_TYPE: usize = 0x10;
    pub(crate) const E_MACHINE: usize = 0x12;
    pub(crate) const E_ENTRY: usize = 0x18;
    pub(crate) const E_PHOFF: usize = 0x20;
    pub(crate) const E_SHOFF: usize = 0x28;
    pub(crate) const E_EHSIZE: usize = 0x34;
    pub(crate) const E_PHENTSIZE: usize = 0x36;
    pub(crate) const E_PHNUM: usize = 0x38;
    pub(crate) const E_SHENTSIZE: usize = 0x3A;
    pub(crate) const E_SHNUM: usize = 0x3C;
}

/// One program header, as the caller wants it described.
struct Segment {
    p_type: u32,
    offset: u64,
    vaddr: u64,
    filesz: u64,
}

/// Builds a module with the given segments, `size` bytes long.
fn module(e_type: u16, segments: &[Segment], size: usize) -> Vec<u8> {
    let phoff = HEADER_SIZE;
    let mut out = vec![0_u8; size.max(phoff + PROGRAM_HEADER_SIZE * segments.len())];
    out[..4].copy_from_slice(&MAGIC);
    out[EI_CLASS] = CLASS64;
    out[EI_DATA] = DATA_LSB;
    out[6] = 1; // EI_VERSION
    out[EI_OSABI] = OSABI_FREEBSD;
    out[EI_ABIVERSION] = Generation::Prospero.abi_version();
    out[at::E_TYPE..at::E_TYPE + 2].copy_from_slice(&e_type.to_le_bytes());
    out[at::E_MACHINE..at::E_MACHINE + 2].copy_from_slice(&MACHINE_X86_64.to_le_bytes());
    out[at::E_ENTRY..at::E_ENTRY + 8].copy_from_slice(&0x1234_u64.to_le_bytes());
    out[at::E_PHOFF..at::E_PHOFF + 8].copy_from_slice(&(phoff as u64).to_le_bytes());
    out[at::E_EHSIZE..at::E_EHSIZE + 2].copy_from_slice(
        &u16::try_from(HEADER_SIZE)
            .expect("the header size fits")
            .to_le_bytes(),
    );
    out[at::E_PHENTSIZE..at::E_PHENTSIZE + 2].copy_from_slice(
        &u16::try_from(PROGRAM_HEADER_SIZE)
            .expect("a program header fits")
            .to_le_bytes(),
    );
    out[at::E_PHNUM..at::E_PHNUM + 2].copy_from_slice(
        &u16::try_from(segments.len())
            .expect("these fixtures declare few segments")
            .to_le_bytes(),
    );

    for (index, seg) in segments.iter().enumerate() {
        let base = phoff + PROGRAM_HEADER_SIZE * index;
        out[base..base + 4].copy_from_slice(&seg.p_type.to_le_bytes());
        out[base + 8..base + 16].copy_from_slice(&seg.offset.to_le_bytes());
        out[base + 16..base + 24].copy_from_slice(&seg.vaddr.to_le_bytes());
        out[base + 32..base + 40].copy_from_slice(&seg.filesz.to_le_bytes());
        out[base + 40..base + 48].copy_from_slice(&seg.filesz.to_le_bytes());
    }
    out
}

/// An ordinary module: one load, one dynamic table, one vendor segment.
fn shaped() -> Vec<u8> {
    let mut bytes = module(
        ObjectType::SHARED_LIBRARY,
        &[
            Segment {
                p_type: segment::LOAD,
                offset: 0,
                vaddr: 0,
                filesz: 0x800,
            },
            Segment {
                p_type: segment::DYNAMIC,
                offset: 0x200,
                vaddr: 0x200,
                filesz: 0x30,
            },
            Segment {
                p_type: segment::SCE_DYNLIBDATA,
                offset: 0x400,
                vaddr: 0,
                filesz: 0x100,
            },
        ],
        0x800,
    );

    // A dynamic table of two entries and a terminator; the walk does not interpret the tags.
    let pairs: [(u64, u64); 3] = [(0x0000_0001, 0x11), (0x6100_0025, 0x22), (0, 0)];
    for (index, (tag, value)) in pairs.iter().enumerate() {
        let base = 0x200 + index * 16;
        bytes[base..base + 8].copy_from_slice(&tag.to_le_bytes());
        bytes[base + 8..base + 16].copy_from_slice(&value.to_le_bytes());
    }

    // Recognisable bytes, so reading the right segment can be told from a wrong one.
    bytes[0x400..0x404].copy_from_slice(b"VEND");
    bytes
}

/// Each way a file can fail to be a module has its own error.
#[test]
fn every_way_of_not_being_a_module_has_its_own_answer() {
    assert!(matches!(
        Elf::parse(&[]),
        Err(ElfError::TooShort { needed, found }) if needed == HEADER_SIZE && found == 0
    ));

    let bytes = shaped();
    assert!(
        matches!(
            Elf::parse(&bytes[..HEADER_SIZE - 1]),
            Err(ElfError::TooShort { .. })
        ),
        "one byte short of a header is short, not unrecognisable"
    );
    // Exactly a header parses only when no segments are declared; with the table cut off it
    // is out of bounds rather than short.
    assert!(matches!(
        Elf::parse(&bytes[..HEADER_SIZE]),
        Err(ElfError::ProgramHeadersOutOfBounds)
    ));
    let mut bare = shaped();
    bare[at::E_PHNUM..at::E_PHNUM + 2].copy_from_slice(&0_u16.to_le_bytes());
    assert!(
        Elf::parse(&bare[..HEADER_SIZE]).is_ok(),
        "a file that declares nothing past its header parses from its header alone"
    );

    let mut not_elf = shaped();
    not_elf[..4].copy_from_slice(&[0x4F, 0x15, 0x3D, 0x1D]);
    assert!(matches!(
        Elf::parse(&not_elf),
        Err(ElfError::NotAnElf([0x4F, 0x15, 0x3D, 0x1D]))
    ));

    let mut thirty_two = shaped();
    thirty_two[EI_CLASS] = 1;
    assert!(matches!(
        Elf::parse(&thirty_two),
        Err(ElfError::NotSixtyFourBit)
    ));

    let mut big_endian = shaped();
    big_endian[EI_DATA] = 2;
    assert!(matches!(
        Elf::parse(&big_endian),
        Err(ElfError::NotLittleEndian)
    ));
}

/// Refusing a container, the message carries the magic bytes it found.
#[test]
fn a_refusal_says_what_the_file_actually_began_with() {
    let mut container = shaped();
    let magic = Generation::Prospero.container_magic();
    container[..4].copy_from_slice(&magic);

    let error = Elf::parse(&container).expect_err("a container is not an ELF");
    let message = error.to_string();
    for byte in magic {
        assert!(
            message.contains(&format!("{byte:02x}")),
            "the message should carry the bytes it found: {message}"
        );
    }
}

/// Every refusal renders as a sentence carrying its number, not a variant name.
#[test]
fn every_refusal_reads_as_a_sentence() {
    let messages = [
        ElfError::TooShort {
            needed: 64,
            found: 3,
        }
        .to_string(),
        ElfError::NotAnElf([1, 2, 3, 4]).to_string(),
        ElfError::NotSixtyFourBit.to_string(),
        ElfError::NotLittleEndian.to_string(),
        ElfError::UnexpectedProgramHeaderSize(32).to_string(),
        ElfError::ProgramHeadersOutOfBounds.to_string(),
        ElfError::UnexpectedObjectType(0x1234).to_string(),
    ];
    for message in &messages {
        assert!(!message.is_empty());
        assert!(
            message.contains(' '),
            "a one-word answer is a variant name, not an explanation: {message}"
        );
    }
    assert!(messages[0].contains("64") && messages[0].contains('3'));
    assert!(messages[4].contains("32"));
    assert!(messages[6].contains("0x1234"));
}

/// A program header table whose count or offset runs past the file is refused.
#[test]
fn a_program_header_table_that_does_not_fit_is_refused() {
    let mut too_many = shaped();
    too_many[at::E_PHNUM..at::E_PHNUM + 2].copy_from_slice(&2000_u16.to_le_bytes());
    assert!(matches!(
        Elf::parse(&too_many),
        Err(ElfError::ProgramHeadersOutOfBounds)
    ));

    let mut far_away = shaped();
    far_away[at::E_PHOFF..at::E_PHOFF + 8].copy_from_slice(&0xFFFF_0000_u64.to_le_bytes());
    assert!(matches!(
        Elf::parse(&far_away),
        Err(ElfError::ProgramHeadersOutOfBounds)
    ));
}

/// A program header entry size other than the format's is refused, not read at that stride.
#[test]
fn a_program_header_of_the_wrong_size_is_refused() {
    let mut wrong = shaped();
    wrong[at::E_PHENTSIZE..at::E_PHENTSIZE + 2].copy_from_slice(&32_u16.to_le_bytes());
    assert!(matches!(
        Elf::parse(&wrong),
        Err(ElfError::UnexpectedProgramHeaderSize(32))
    ));

    // With no segments declared the entry size is not checked.
    let mut none = shaped();
    none[at::E_PHNUM..at::E_PHNUM + 2].copy_from_slice(&0_u16.to_le_bytes());
    none[at::E_PHENTSIZE..at::E_PHENTSIZE + 2].copy_from_slice(&0_u16.to_le_bytes());
    let elf = Elf::parse(&none).expect("a module with no segments is still a module");
    assert!(elf.program_headers().is_empty());
}

/// The header fields are reported as the file states them.
#[test]
fn the_header_is_reported_as_it_stands() {
    let bytes = shaped();
    let elf = Elf::parse(&bytes).expect("parses");

    assert_eq!(elf.bytes().len(), bytes.len(), "the whole file is kept");
    assert_eq!(elf.entry(), 0x1234);
    assert_eq!(elf.header().machine.get(), MACHINE_X86_64);
    assert_eq!(elf.object_type(), ObjectType::SharedLibrary);
    assert!(elf.has_platform_osabi());
    assert_eq!(elf.generation(), Some(Generation::Prospero));
    assert_eq!(elf.program_headers().len(), 3);
}

/// An ordinary ELF is described rather than refused, and told apart by OSABI and `e_type`.
#[test]
fn an_ordinary_elf_is_described_rather_than_refused() {
    let mut bytes = shaped();
    bytes[EI_OSABI] = 0; // System V
    bytes[EI_ABIVERSION] = 0;
    bytes[at::E_TYPE..at::E_TYPE + 2].copy_from_slice(&3_u16.to_le_bytes()); // ET_DYN

    let elf = Elf::parse(&bytes).expect("an ordinary ELF still parses");
    assert!(!elf.has_platform_osabi());
    assert_eq!(
        elf.generation(),
        Some(Generation::Orbis),
        "zero is the Orbis-generation value, so this byte alone does not tell them apart"
    );
    assert!(
        !elf.has_platform_osabi() && !elf.object_type().is_platform(),
        "an ordinary object carries neither the platform's OSABI nor one of its e_types"
    );
    assert_eq!(elf.object_type(), ObjectType::Other(3));
    assert!(!elf.object_type().is_platform());
    assert!(!elf.object_type().is_executable());
}

/// Each object type displays, round-trips and classifies itself correctly.
#[test]
fn each_object_type_describes_itself() {
    for (raw, text, platform, executable) in [
        (
            ObjectType::FIXED_EXECUTABLE,
            "fixed-address executable",
            true,
            true,
        ),
        (ObjectType::EXECUTABLE, "executable", true, true),
        (ObjectType::SHARED_LIBRARY, "shared library", true, false),
    ] {
        let kind = ObjectType::from_raw(raw);
        assert_eq!(kind.to_string(), text);
        assert_eq!(kind.to_raw(), raw, "round-trips through the raw value");
        assert_eq!(kind.is_platform(), platform);
        assert_eq!(
            kind.is_executable(),
            executable,
            "{text}: a loader runs a library's initialisers and looks elsewhere for an \
             entry point"
        );
    }

    // Anything else keeps its number.
    let other = ObjectType::from_raw(0x0003);
    assert_eq!(other.to_string(), "e_type 0x0003");
    assert_eq!(other.to_raw(), 0x0003);
    assert!(!other.is_platform());
}

/// A segment is found by type, and its bytes come back only if they are in the file.
#[test]
fn a_segment_is_found_by_type_and_read_only_if_it_is_there() {
    let bytes = shaped();
    let elf = Elf::parse(&bytes).expect("parses");

    let dynamic = elf.segment(segment::DYNAMIC).expect("there is one");
    assert_eq!(dynamic.offset.get(), 0x200);
    assert_eq!(
        elf.segment_bytes(dynamic).expect("within the file").len(),
        0x30
    );

    assert!(
        elf.segment(segment::INTERP).is_none(),
        "a type this module does not carry is absent, not an error"
    );
}

/// A segment describing more than the file holds reads as absent, without a panic.
#[test]
fn a_segment_past_the_end_of_the_file_is_not_read() {
    let bytes = module(
        ObjectType::SHARED_LIBRARY,
        &[Segment {
            p_type: segment::LOAD,
            offset: 0x8000,
            vaddr: 0,
            filesz: 0x1000,
        }],
        0x200,
    );
    let elf = Elf::parse(&bytes).expect("the header is still valid");
    let load = elf.segment(segment::LOAD).expect("declared");
    assert_eq!(
        elf.segment_bytes(load),
        None,
        "a header may describe more than the file holds"
    );
}

/// The vendor segment is found and read whole.
#[test]
fn the_vendor_segment_is_found_and_read() {
    let bytes = shaped();
    let elf = Elf::parse(&bytes).expect("parses");

    let vendor = elf.vendor_segment().expect("this module carries one");
    assert_eq!(vendor.len(), 0x100);
    assert_eq!(&vendor[..4], b"VEND", "and it is the right segment");
}

/// A module with no vendor segment reports `None` and an empty dynamic table.
#[test]
fn a_module_without_a_vendor_segment_reports_none() {
    let bytes = module(
        ObjectType::EXECUTABLE,
        &[Segment {
            p_type: segment::LOAD,
            offset: 0,
            vaddr: 0,
            filesz: 0x100,
        }],
        0x200,
    );
    let elf = Elf::parse(&bytes).expect("parses");
    assert_eq!(elf.vendor_segment(), None);
    assert_eq!(
        elf.dynamic_entries().expect("walks"),
        Vec::new(),
        "no dynamic segment is an empty table, not a failure"
    );
    assert_eq!(
        elf.tables().expect("walks"),
        None,
        "and no vendor tables, as for an ordinary ELF"
    );
}

/// The dynamic table stops at its terminator, and unknown tags are kept rather than refused.
#[test]
fn the_dynamic_table_stops_at_its_terminator() {
    let bytes = shaped();
    let elf = Elf::parse(&bytes).expect("parses");

    let entries = elf.dynamic_entries().expect("walks");
    assert_eq!(
        entries,
        vec![(0x0000_0001, 0x11), (0x6100_0025, 0x22)],
        "both pairs, and nothing past the zero tag"
    );
}

/// `header_span` covers the program header table, and never less than the file header.
#[test]
fn the_header_span_covers_the_table_or_the_header_whichever_is_larger() {
    let bytes = shaped();
    let elf = Elf::parse(&bytes).expect("parses");
    assert_eq!(
        elf.header_span(),
        u64::try_from(HEADER_SIZE + PROGRAM_HEADER_SIZE * 3).expect("fits")
    );

    let mut none = shaped();
    none[at::E_PHNUM..at::E_PHNUM + 2].copy_from_slice(&0_u16.to_le_bytes());
    let bare = Elf::parse(&none).expect("parses");
    assert_eq!(
        bare.header_span(),
        u64::try_from(HEADER_SIZE).expect("fits"),
        "with no segments the header itself is the whole span"
    );
}

/// No section table reads as `None`, while a declared but unreadable one is an error.
#[test]
fn a_module_with_no_sections_is_not_a_failure() {
    let bytes = shaped();
    let elf = Elf::parse(&bytes).expect("parses");
    assert!(
        elf.sections().expect("asking is not an error").is_none(),
        "a finished module normally has none at all"
    );

    let mut claims = shaped();
    claims[at::E_SHOFF..at::E_SHOFF + 8].copy_from_slice(&0xFFFF_0000_u64.to_le_bytes());
    claims[at::E_SHNUM..at::E_SHNUM + 2].copy_from_slice(&4_u16.to_le_bytes());
    claims[at::E_SHENTSIZE..at::E_SHENTSIZE + 2].copy_from_slice(&64_u16.to_le_bytes());
    let claiming = Elf::parse(&claims).expect("the header is still valid");
    assert!(
        claiming.sections().is_err(),
        "a section table that is present and unreadable is reported"
    );
}

/// Vendor segment types are classed as vendor and GNU ones in the same range are not.
#[test]
fn a_gnu_segment_is_not_vendor_data() {
    for vendor in [
        segment::SCE_RELA,
        segment::SCE_DYNLIBDATA,
        segment::SCE_PROCPARAM,
        segment::SCE_MODULE_PARAM,
        segment::SCE_RELRO,
        segment::SCE_COMMENT,
        segment::SCE_VERSION,
    ] {
        assert!(segment::is_vendor(vendor), "{vendor:#x} is the vendor's");
        assert!(
            segment::OS_SPECIFIC.contains(&vendor),
            "{vendor:#x} should be inside the OS-specific range"
        );
    }

    for gnu in segment::GNU {
        assert!(
            segment::OS_SPECIFIC.contains(&gnu),
            "{gnu:#x} is inside the range"
        );
        assert!(!segment::is_vendor(gnu), "{gnu:#x} is a GNU extension");
    }

    for ordinary in [
        segment::LOAD,
        segment::DYNAMIC,
        segment::INTERP,
        segment::TLS,
    ] {
        assert!(!segment::is_vendor(ordinary));
    }
    assert!(
        !segment::is_vendor(0x7000_0000),
        "past the top of the range is not the vendor's either"
    );
}

/// Only an `EI_ABIVERSION` belonging to neither generation reads as `None`.
#[test]
fn only_a_byte_belonging_to_neither_generation_reads_as_neither() {
    let mut bytes = shaped();
    for (value, want) in [
        (
            Generation::Prospero.abi_version(),
            Some(Generation::Prospero),
        ),
        (Generation::Orbis.abi_version(), Some(Generation::Orbis)),
        (1, None),
        (3, None),
        (0xFF, None),
    ] {
        bytes[EI_ABIVERSION] = value;
        let elf = Elf::parse(&bytes).expect("parses");
        assert_eq!(elf.generation(), want, "EI_ABIVERSION = {value}");
    }
}
