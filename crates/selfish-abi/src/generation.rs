//! Which hardware generation a file is built for.
//!
//! The two generations share one container and differ in its four magic bytes; header size,
//! segment size, field offsets and flag bits are identical. A file built for the wrong
//! generation is structurally valid and rejected on its first four bytes, which a loader
//! reports as "not a container".
//!
//! There is no `Default`: a builder states its target generation or does not compile (D002).

use core::fmt;

/// Which hardware generation a file is built for, or was found to be built for.
///
/// Has no `Default`; see the module note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Generation {
    /// Prospero-generation hardware.
    Prospero,
    /// Orbis-generation hardware.
    Orbis,
}

impl Generation {
    /// The container magic, as the four bytes that appear at offset zero.
    ///
    /// Bytes rather than an integer, so the written form carries no endianness to get wrong.
    #[must_use]
    pub const fn container_magic(self) -> [u8; 4] {
        match self {
            Self::Prospero => [0x54, 0x14, 0xF5, 0xEE],
            Self::Orbis => [0x4F, 0x15, 0x3D, 0x1D],
        }
    }

    /// Which generation a file claims, from the four bytes at offset zero.
    ///
    /// `None` means the bytes match neither generation: the file is not a container, which is
    /// distinct from a container for the other generation.
    #[must_use]
    pub fn from_container_magic(bytes: [u8; 4]) -> Option<Self> {
        if bytes == Self::Prospero.container_magic() {
            Some(Self::Prospero)
        } else if bytes == Self::Orbis.container_magic() {
            Some(Self::Orbis)
        } else {
            None
        }
    }

    /// The `EI_ABIVERSION` byte an executable carries for this generation.
    ///
    /// A loader reads it before any guest instruction runs, so it is fixed at build time.
    #[must_use]
    pub const fn abi_version(self) -> u8 {
        match self {
            Self::Prospero => 2,
            Self::Orbis => 0,
        }
    }

    /// The generation number used by build flags.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Prospero => 5,
            Self::Orbis => 4,
        }
    }

    /// The generation for a build-flag number.
    ///
    /// Any other number is `None`, never a fallback to either generation.
    #[must_use]
    pub const fn from_number(n: u8) -> Option<Self> {
        match n {
            5 => Some(Self::Prospero),
            4 => Some(Self::Orbis),
            _ => None,
        }
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prospero => write!(f, "prospero"),
            Self::Orbis => write!(f, "orbis"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Generation;

    /// The magic is checked in its written byte order, which a `u32` comparison would not.
    #[test]
    fn the_magic_is_asserted_as_written_bytes() {
        assert_eq!(
            Generation::Prospero.container_magic(),
            [0x54, 0x14, 0xF5, 0xEE]
        );
        assert_eq!(
            Generation::Orbis.container_magic(),
            [0x4F, 0x15, 0x3D, 0x1D]
        );
    }

    /// Each generation's magic is distinct and reads back as that generation.
    #[test]
    fn the_two_generations_are_never_confused_for_each_other() {
        assert_ne!(
            Generation::Prospero.container_magic(),
            Generation::Orbis.container_magic()
        );
        assert_eq!(
            Generation::from_container_magic([0x54, 0x14, 0xF5, 0xEE]),
            Some(Generation::Prospero)
        );
        assert_eq!(
            Generation::from_container_magic([0x4F, 0x15, 0x3D, 0x1D]),
            Some(Generation::Orbis)
        );
    }

    /// A plain ELF or zero bytes read as no container, not as either generation.
    #[test]
    fn bytes_matching_neither_are_not_a_container_rather_than_a_wrong_one() {
        assert_eq!(
            Generation::from_container_magic([0x7F, 0x45, 0x4C, 0x46]),
            None
        );
        assert_eq!(Generation::from_container_magic([0; 4]), None);
    }

    /// Every generation round-trips through its magic.
    #[test]
    fn a_generation_survives_a_round_trip_through_its_magic() {
        for g in [Generation::Prospero, Generation::Orbis] {
            assert_eq!(
                Generation::from_container_magic(g.container_magic()),
                Some(g)
            );
        }
    }

    /// The build-flag number round-trips, and other numbers map to no generation.
    #[test]
    fn the_build_flag_number_round_trips_and_refuses_anything_else() {
        for g in [Generation::Prospero, Generation::Orbis] {
            assert_eq!(Generation::from_number(g.number()), Some(g));
        }
        assert_eq!(Generation::from_number(3), None);
        assert_eq!(Generation::from_number(0), None);
    }

    /// Each generation carries its own `EI_ABIVERSION` byte.
    #[test]
    fn abi_version_differs_because_a_loader_reads_it_before_anything_runs() {
        assert_eq!(Generation::Prospero.abi_version(), 2);
        assert_eq!(Generation::Orbis.abi_version(), 0);
    }
}
