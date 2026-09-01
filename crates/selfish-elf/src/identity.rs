//! Stamping the header fields a loader checks before it reads anything else.
//!
//! Three bytes' worth of `e_ident` and `e_type`, and getting any of them wrong means the file
//! is refused outright - with a message about the header rather than about anything the module
//! does. No linker sets them, because no linker knows about either console.
//!
//! ```text
//! IsElfFile: e_ident[EI_OSABI] expected 0x09 is (0x0)
//! IsElfFile: e_type expected 0xFE10 OR 0xFE18 OR 0xfe00 is (0x3)
//! ```
//!
//! This is the writing side of what [`crate::Elf::object_type`], [`crate::Elf::generation`] and
//! [`crate::Elf::has_platform_osabi`] read.
//!
//! # The object type is a parameter, and deliberately so
//!
//! An executable and a shared library are both legitimate outputs, they are different files,
//! and only the builder knows which it is making. A stamper that hardcodes one is right for
//! exactly one consumer.
//!
//! It also happens to be the field with the worst history here. The two constants were named
//! the wrong way round for months in a sibling project, so a builder wrote "shared library"
//! while its own log said "executable" - and the symptom was precisely what you would predict
//! and nobody did: a loader mapped the module, ran its initialisers, then looked elsewhere for
//! a process to start. It loads, and it never runs. See [`crate::ObjectType`].

use crate::{EI_ABIVERSION, EI_OSABI, ElfError, Generation, OSABI_FREEBSD, ObjectType};

/// Offset of `e_type` in the file header.
pub const E_TYPE: usize = 0x10;

/// An ordinary shared object, which is what a linker produces.
pub const ET_DYN: u16 = 0x0003;

/// One field this changed.
///
/// Returned rather than logged, so a caller can print exactly what it did to somebody's file.
/// A tool that silently rewrites header bytes is one nobody can debug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    /// Which field.
    pub field: &'static str,
    /// What it held.
    pub from: u64,
    /// What it holds now.
    pub to: u64,
}

/// Stamp the platform's identity onto a linked module, in place.
///
/// Idempotent: a field already correct is left alone and produces no [`Change`], so running it
/// twice reports nothing the second time.
///
/// # Errors
///
/// If the file ends inside its own header, or if `e_type` is neither what a linker produces nor
/// one of the two the platform accepts - because rewriting an unrecognised type would assert
/// something untrue about a file this code does not understand.
pub fn stamp(
    bytes: &mut [u8],
    object_type: ObjectType,
    generation: Generation,
) -> Result<Vec<Change>, ElfError> {
    let mut changes = Vec::new();
    // Taken up front: every bounds check below reports it, and reading it inside a `get_mut`
    // would borrow the slice twice.
    let found = bytes.len();

    // `lld` targeting FreeBSD sets this correctly and GNU `ld` does not, which is how it was
    // first seen - and is why a build that cares pins its linker rather than relying on this.
    let osabi = *bytes.get(EI_OSABI).ok_or(ElfError::TooShort {
        needed: EI_OSABI.saturating_add(1),
        found,
    })?;
    if osabi != OSABI_FREEBSD {
        changes.push(Change {
            field: "EI_OSABI",
            from: u64::from(osabi),
            to: u64::from(OSABI_FREEBSD),
        });
        *bytes.get_mut(EI_OSABI).ok_or(ElfError::TooShort {
            needed: EI_OSABI.saturating_add(1),
            found,
        })? = OSABI_FREEBSD;
    }

    // Which generation the module claims to be for.
    //
    // **Not a constant, because the loaders disagree and both are right.** One reads 2 as the
    // current generation; another is a previous-generation emulator that refuses anything but
    // 0. A module claiming the wrong one is lying to whichever it meets, so the caller says.
    let wanted = generation.abi_version();
    let held = *bytes.get(EI_ABIVERSION).ok_or(ElfError::TooShort {
        needed: EI_ABIVERSION.saturating_add(1),
        found,
    })?;
    if held != wanted {
        changes.push(Change {
            field: "EI_ABIVERSION",
            from: u64::from(held),
            to: u64::from(wanted),
        });
        *bytes.get_mut(EI_ABIVERSION).ok_or(ElfError::TooShort {
            needed: EI_ABIVERSION.saturating_add(1),
            found,
        })? = wanted;
    }

    // `Other` is refused rather than written through. It is how the reader represents "not a
    // platform type", and passing one here means the caller is asking to stamp a value this
    // code cannot vouch for.
    if let ObjectType::Other(raw) = object_type {
        return Err(ElfError::UnexpectedObjectType(raw));
    }
    let target = object_type.to_raw();
    let slice = bytes
        .get_mut(E_TYPE..E_TYPE.saturating_add(2))
        .ok_or(ElfError::TooShort {
            needed: E_TYPE.saturating_add(2),
            found,
        })?;
    let mut raw = [0_u8; 2];
    raw.copy_from_slice(slice);
    let held = u16::from_le_bytes(raw);

    if held == target {
        return Ok(changes);
    }
    // Rewritable from what a linker produces, or from one platform type to another. Anything
    // else means the link produced something this code does not understand, and stamping a
    // type onto it would assert something untrue - a relocatable object called an executable.
    //
    // The three platform values are the three a loader names when it refuses:
    // `e_type expected 0xFE10 OR 0xFE18 OR 0xfe00`.
    if held != ET_DYN && !ObjectType::from_raw(held).is_platform() {
        return Err(ElfError::UnexpectedObjectType(held));
    }
    changes.push(Change {
        field: "e_type",
        from: u64::from(held),
        to: u64::from(target),
    });
    slice.copy_from_slice(&target.to_le_bytes());
    Ok(changes)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{Change, E_TYPE, ET_DYN, stamp};
    use crate::{EI_ABIVERSION, EI_OSABI, ElfError, Generation, OSABI_FREEBSD, ObjectType};

    /// A header as a linker leaves it: `SysV` ABI, version zero, an ordinary shared object.
    fn linked() -> Vec<u8> {
        let mut bytes = vec![0_u8; 64];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2; // 64-bit
        bytes[5] = 1; // little-endian
        bytes[E_TYPE..E_TYPE + 2].copy_from_slice(&ET_DYN.to_le_bytes());
        bytes
    }

    fn e_type(bytes: &[u8]) -> u16 {
        u16::from_le_bytes([bytes[E_TYPE], bytes[E_TYPE + 1]])
    }

    #[test]
    fn a_linked_object_gets_all_three_fields() {
        // The current generation, because the previous one's ABI version is zero - which is
        // what a linker already leaves, so targeting it changes only two fields. That is
        // correct and it is a poor test of "all three".
        let mut bytes = linked();
        let changes =
            stamp(&mut bytes, ObjectType::Executable, Generation::Current).expect("stamped");

        assert_eq!(changes.len(), 3, "{changes:?}");
        assert_eq!(bytes[EI_OSABI], OSABI_FREEBSD);
        assert_eq!(bytes[EI_ABIVERSION], Generation::Current.abi_version());
        assert_eq!(e_type(&bytes), ObjectType::EXECUTABLE);
    }

    #[test]
    fn targeting_the_previous_generation_leaves_the_abi_version_alone() {
        // Its ABI version is zero, which is what a linker leaves. Reporting a change here
        // would be a tool claiming credit for a byte it did not touch.
        let mut bytes = linked();
        let changes =
            stamp(&mut bytes, ObjectType::Executable, Generation::Previous).expect("stamped");

        assert_eq!(changes.len(), 2, "{changes:?}");
        assert!(!changes.iter().any(|change| change.field == "EI_ABIVERSION"));
        assert_eq!(bytes[EI_ABIVERSION], 0);
    }

    #[test]
    fn the_object_type_is_the_callers_choice_and_not_a_constant() {
        // An executable and a shared library are both legitimate outputs and they are
        // different files. The two constants were named the wrong way round for months in a
        // sibling project, and the result loaded, ran its initialisers, and was never entered.
        let mut executable = linked();
        stamp(&mut executable, ObjectType::Executable, Generation::Current).expect("stamped");
        let mut library = linked();
        stamp(&mut library, ObjectType::SharedLibrary, Generation::Current).expect("stamped");

        assert_eq!(e_type(&executable), 0xFE10);
        assert_eq!(e_type(&library), 0xFE18);
        assert_ne!(e_type(&executable), e_type(&library));
    }

    #[test]
    fn the_generation_comes_from_the_caller_because_the_loaders_disagree() {
        // One loader reads 2 as the current generation; another refuses anything but 0. A
        // module claiming the wrong one is lying to whichever it meets.
        let mut current = linked();
        stamp(&mut current, ObjectType::Executable, Generation::Current).expect("stamped");
        let mut previous = linked();
        stamp(&mut previous, ObjectType::Executable, Generation::Previous).expect("stamped");

        assert_ne!(current[EI_ABIVERSION], previous[EI_ABIVERSION]);
    }

    #[test]
    fn stamping_twice_changes_nothing_the_second_time() {
        let mut bytes = linked();
        stamp(&mut bytes, ObjectType::Executable, Generation::Previous).expect("stamped");
        let again =
            stamp(&mut bytes, ObjectType::Executable, Generation::Previous).expect("stamped");
        assert!(again.is_empty(), "{again:?}");
    }

    #[test]
    fn restamping_from_one_vendor_type_to_the_other_is_allowed_and_reported() {
        // Changing a library into an executable is a real thing to want, and the change is
        // reported so a caller can say it happened.
        let mut bytes = linked();
        stamp(&mut bytes, ObjectType::SharedLibrary, Generation::Previous).expect("stamped");
        let changes =
            stamp(&mut bytes, ObjectType::Executable, Generation::Previous).expect("stamped");

        assert_eq!(
            changes,
            vec![Change {
                field: "e_type",
                from: 0xFE18,
                to: 0xFE10,
            }]
        );
    }

    #[test]
    fn an_unrecognised_type_is_refused_rather_than_overwritten() {
        // Rewriting it would assert something untrue about a file this code does not
        // understand - a relocatable object stamped as an executable, say.
        let mut bytes = linked();
        bytes[E_TYPE..E_TYPE + 2].copy_from_slice(&1_u16.to_le_bytes()); // ET_REL
        assert!(matches!(
            stamp(&mut bytes, ObjectType::Executable, Generation::Previous),
            Err(ElfError::UnexpectedObjectType(1))
        ));
    }

    #[test]
    fn a_truncated_header_is_an_error_rather_than_a_partial_stamp() {
        let mut bytes = vec![0_u8; 8];
        assert!(stamp(&mut bytes, ObjectType::Executable, Generation::Previous).is_err());
    }

    #[test]
    fn what_is_stamped_reads_back_through_the_reader() {
        let mut bytes = linked();
        stamp(&mut bytes, ObjectType::SharedLibrary, Generation::Current).expect("stamped");

        // Enough of a header for the parser: phnum stays zero, so there is no table to read.
        let elf = crate::Elf::parse(&bytes).expect("a readable header");
        assert_eq!(elf.object_type(), ObjectType::SharedLibrary);
        assert_eq!(elf.generation(), Some(Generation::Current));
        assert!(elf.has_platform_osabi());
    }
}
