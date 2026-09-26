//! Stamping the header fields a loader checks before it reads anything else.
//!
//! `EI_OSABI`, `EI_ABIVERSION` and `e_type` are not set by a linker, and a wrong value is
//! refused with a header message:
//!
//! ```text
//! IsElfFile: e_ident[EI_OSABI] expected 0x09 is (0x0)
//! IsElfFile: e_type expected 0xFE10 OR 0xFE18 OR 0xfe00 is (0x3)
//! ```
//!
//! This is the writing side of [`crate::Elf::object_type`], [`crate::Elf::generation`] and
//! [`crate::Elf::has_platform_osabi`]. The object type is a parameter because only the builder
//! knows whether it makes an executable or a shared library; see [`crate::ObjectType`].

use selfish_bytes::{read_le, write_le};

use crate::{EI_ABIVERSION, EI_OSABI, ElfError, Generation, OSABI_FREEBSD, ObjectType};

/// Offset of `e_type` in the file header.
pub const E_TYPE: usize = 0x10;

/// An ordinary shared object, which is what a linker produces.
pub const ET_DYN: u16 = 0x0003;

/// One field this changed.
///
/// Returned rather than logged, so a caller can report exactly what it rewrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    /// Which field.
    pub field: &'static str,
    /// What it held.
    pub from: u64,
    /// What it holds after stamping.
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
/// a platform type, since the file is then not one this code understands.
pub fn stamp(
    bytes: &mut [u8],
    object_type: ObjectType,
    generation: Generation,
) -> Result<Vec<Change>, ElfError> {
    let mut changes = Vec::new();
    // Taken up front: reading it inside a `get_mut` would borrow the slice twice.
    let found = bytes.len();

    // `lld` targeting FreeBSD sets this; GNU `ld` does not.
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

    // The caller chooses: a Prospero-generation loader reads 2, an Orbis-generation one
    // refuses anything but 0.
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

    // `Other` is not a platform type, so it is refused rather than written.
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
    // Rewritable only from what a linker produces or from another platform type; anything
    // else, such as a relocatable object, is refused.
    if held != ET_DYN && !ObjectType::from_raw(held).is_platform() {
        return Err(ElfError::UnexpectedObjectType(held));
    }
    changes.push(Change {
        field: "e_type",
        from: u64::from(held),
        to: u64::from(target),
    });
    slice.copy_from_slice(&target.to_le_bytes());

    if generation == Generation::Prospero {
        stamp_prospero_segments(bytes, &mut changes);
    }

    Ok(changes)
}

/// Make every read-and-execute load segment execute-only. On Prospero-generation hardware
/// `rtld` refuses an R+X segment in a native program or PRX.
fn stamp_prospero_segments(bytes: &mut [u8], changes: &mut Vec<Change>) {
    if bytes.len() < 64 {
        return;
    }
    let phoff = read_le::<u64>(bytes, 0x20)
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or(0);
    let phnum = read_le::<u16>(bytes, 0x38).map_or(0, usize::from);
    let phentsize = read_le::<u16>(bytes, 0x36).map_or(0, usize::from);
    if phoff == 0 || phentsize < 56 {
        return;
    }
    for i in 0..phnum {
        let entry_at = phoff.saturating_add(i.saturating_mul(phentsize));
        let flags_at = entry_at.saturating_add(4);
        let (Some(p_type), Some(p_flags)) = (
            read_le::<u32>(bytes, entry_at),
            read_le::<u32>(bytes, flags_at),
        ) else {
            continue;
        };
        if p_type == crate::segment::LOAD
            && p_flags == 5
            && write_le(bytes, flags_at, 1_u32).is_some()
        {
            changes.push(Change {
                field: "p_flags (R+X -> X)",
                from: 5,
                to: 1,
            });
        }
    }
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

    /// Stamping a linked object for Prospero changes all three identity fields.
    #[test]
    fn a_linked_object_gets_all_three_fields() {
        let mut bytes = linked();
        let changes =
            stamp(&mut bytes, ObjectType::Executable, Generation::Prospero).expect("stamped");

        assert_eq!(changes.len(), 3, "{changes:?}");
        assert_eq!(bytes[EI_OSABI], OSABI_FREEBSD);
        assert_eq!(bytes[EI_ABIVERSION], Generation::Prospero.abi_version());
        assert_eq!(e_type(&bytes), ObjectType::EXECUTABLE);
    }

    /// Targeting Orbis leaves the zero ABI version untouched and unreported.
    #[test]
    fn targeting_the_orbis_generation_leaves_the_abi_version_alone() {
        let mut bytes = linked();
        let changes =
            stamp(&mut bytes, ObjectType::Executable, Generation::Orbis).expect("stamped");

        assert_eq!(changes.len(), 2, "{changes:?}");
        assert!(!changes.iter().any(|change| change.field == "EI_ABIVERSION"));
        assert_eq!(bytes[EI_ABIVERSION], 0);
    }

    /// Executable and shared library stamp their own distinct `e_type` values.
    #[test]
    fn the_object_type_is_the_callers_choice_and_not_a_constant() {
        let mut executable = linked();
        stamp(
            &mut executable,
            ObjectType::Executable,
            Generation::Prospero,
        )
        .expect("stamped");
        let mut library = linked();
        stamp(
            &mut library,
            ObjectType::SharedLibrary,
            Generation::Prospero,
        )
        .expect("stamped");

        assert_eq!(e_type(&executable), 0xFE10);
        assert_eq!(e_type(&library), 0xFE18);
        assert_ne!(e_type(&executable), e_type(&library));
    }

    /// The two generations stamp different ABI versions.
    #[test]
    fn the_generation_comes_from_the_caller_because_the_loaders_disagree() {
        let mut prospero = linked();
        stamp(&mut prospero, ObjectType::Executable, Generation::Prospero).expect("stamped");
        let mut orbis = linked();
        stamp(&mut orbis, ObjectType::Executable, Generation::Orbis).expect("stamped");

        assert_ne!(prospero[EI_ABIVERSION], orbis[EI_ABIVERSION]);
    }

    /// Stamping is idempotent.
    #[test]
    fn stamping_twice_changes_nothing_the_second_time() {
        let mut bytes = linked();
        stamp(&mut bytes, ObjectType::Executable, Generation::Orbis).expect("stamped");
        let again = stamp(&mut bytes, ObjectType::Executable, Generation::Orbis).expect("stamped");
        assert!(again.is_empty(), "{again:?}");
    }

    /// Restamping one platform type as another is allowed and reported.
    #[test]
    fn restamping_from_one_vendor_type_to_the_other_is_allowed_and_reported() {
        let mut bytes = linked();
        stamp(&mut bytes, ObjectType::SharedLibrary, Generation::Orbis).expect("stamped");
        let changes =
            stamp(&mut bytes, ObjectType::Executable, Generation::Orbis).expect("stamped");

        assert_eq!(
            changes,
            vec![Change {
                field: "e_type",
                from: 0xFE18,
                to: 0xFE10,
            }]
        );
    }

    /// An `e_type` that is neither `ET_DYN` nor a platform type is refused.
    #[test]
    fn an_unrecognised_type_is_refused_rather_than_overwritten() {
        let mut bytes = linked();
        bytes[E_TYPE..E_TYPE + 2].copy_from_slice(&1_u16.to_le_bytes()); // ET_REL
        assert!(matches!(
            stamp(&mut bytes, ObjectType::Executable, Generation::Orbis),
            Err(ElfError::UnexpectedObjectType(1))
        ));
    }

    /// A truncated header is an error, not a partial stamp.
    #[test]
    fn a_truncated_header_is_an_error_rather_than_a_partial_stamp() {
        let mut bytes = vec![0_u8; 8];
        assert!(stamp(&mut bytes, ObjectType::Executable, Generation::Orbis).is_err());
    }

    /// What is stamped reads back through `Elf`.
    #[test]
    fn what_is_stamped_reads_back_through_the_reader() {
        let mut bytes = linked();
        stamp(&mut bytes, ObjectType::SharedLibrary, Generation::Prospero).expect("stamped");

        // Enough of a header for the parser: phnum stays zero, so there is no table to read.
        let elf = crate::Elf::parse(&bytes).expect("a readable header");
        assert_eq!(elf.object_type(), ObjectType::SharedLibrary);
        assert_eq!(elf.generation(), Some(Generation::Prospero));
        assert!(elf.has_platform_osabi());
    }
}
