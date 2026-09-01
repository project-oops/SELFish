//! Which console a file is built for.
//!
//! Two generations share one container and differ in four bytes. That is the entire
//! practical difference at this layer - header size, segment size, field offsets and the
//! flag bits are all identical - which is exactly what makes it dangerous. A file built for
//! the wrong generation is structurally perfect and rejected on its first four bytes, and a
//! loader reports that as "not a container" rather than as "wrong generation".
//!
//! # Why this is a type and not a parameter with a default
//!
//! It was a parameter with a default once, in a sibling project, and the default was the
//! previous generation because every published source describes that one. The current
//! generation's magic had been observed and written down in *another* sibling project, and
//! nothing connected the two. The builder produced files that would have been refused by the
//! only machine they were built for.
//!
//! So there is no `Default` here, deliberately. Constructing one requires saying which, and
//! a caller that does not know which console it is targeting has a question to answer rather
//! than a value to omit.

use core::fmt;

/// Which console generation a file is built for, or was found to be built for.
///
/// Deliberately has no `Default`: see the module note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Generation {
    /// The current console.
    Current,
    /// The previous console. Most published material describes this one.
    Previous,
}

impl Generation {
    /// The container magic, as the four bytes that appear at offset zero.
    ///
    /// Returned as bytes rather than as an integer on purpose. Held as a `u32` this is a
    /// constant that looks right and serialises backwards - which happened, and was caught
    /// only because a test asserted on the written form. Bytes cannot be got the wrong way
    /// round by an endianness mistake because there is no endianness left to get wrong.
    #[must_use]
    pub const fn container_magic(self) -> [u8; 4] {
        match self {
            Self::Current => [0x54, 0x14, 0xF5, 0xEE],
            Self::Previous => [0x4F, 0x15, 0x3D, 0x1D],
        }
    }

    /// Which generation a file claims, from the four bytes at offset zero.
    ///
    /// `None` means the bytes match neither, which is a different finding from "wrong
    /// generation" and worth keeping distinct: one is a file for the other console, the
    /// other is not a container at all.
    #[must_use]
    pub fn from_container_magic(bytes: [u8; 4]) -> Option<Self> {
        if bytes == Self::Current.container_magic() {
            Some(Self::Current)
        } else if bytes == Self::Previous.container_magic() {
            Some(Self::Previous)
        } else {
            None
        }
    }

    /// The `EI_ABIVERSION` byte an executable carries for this generation.
    ///
    /// Read by a loader before a single guest instruction runs, so it cannot be negotiated
    /// and has to be decided when the file is built.
    #[must_use]
    pub const fn abi_version(self) -> u8 {
        match self {
            Self::Current => 2,
            Self::Previous => 0,
        }
    }

    /// The number people building this think in, as used by build flags.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Current => 5,
            Self::Previous => 4,
        }
    }

    /// From that number.
    ///
    /// Anything else is `None` rather than a fallback. A third value is a typo, not a third
    /// console, and quietly treating it as one generation or the other is how a file ends up
    /// built for a machine nobody asked for.
    #[must_use]
    pub const fn from_number(n: u8) -> Option<Self> {
        match n {
            5 => Some(Self::Current),
            4 => Some(Self::Previous),
            _ => None,
        }
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Current => write!(f, "current generation"),
            Self::Previous => write!(f, "previous generation"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Generation;

    #[test]
    fn the_magic_is_asserted_as_written_bytes() {
        // The form that can fail. Asserting on a `u32` constant would pass whatever the
        // endianness, which is precisely the mistake being guarded against.
        assert_eq!(
            Generation::Current.container_magic(),
            [0x54, 0x14, 0xF5, 0xEE]
        );
        assert_eq!(
            Generation::Previous.container_magic(),
            [0x4F, 0x15, 0x3D, 0x1D]
        );
    }

    #[test]
    fn the_two_generations_are_never_confused_for_each_other() {
        assert_ne!(
            Generation::Current.container_magic(),
            Generation::Previous.container_magic()
        );
        assert_eq!(
            Generation::from_container_magic([0x54, 0x14, 0xF5, 0xEE]),
            Some(Generation::Current)
        );
        assert_eq!(
            Generation::from_container_magic([0x4F, 0x15, 0x3D, 0x1D]),
            Some(Generation::Previous)
        );
    }

    #[test]
    fn bytes_matching_neither_are_not_a_container_rather_than_a_wrong_one() {
        // `\x7fELF` - a plain executable, which is a real thing to be handed and a different
        // finding from a container built for the other console.
        assert_eq!(
            Generation::from_container_magic([0x7F, 0x45, 0x4C, 0x46]),
            None
        );
        assert_eq!(Generation::from_container_magic([0; 4]), None);
    }

    #[test]
    fn a_generation_survives_a_round_trip_through_its_magic() {
        for g in [Generation::Current, Generation::Previous] {
            assert_eq!(
                Generation::from_container_magic(g.container_magic()),
                Some(g)
            );
        }
    }

    #[test]
    fn the_build_flag_number_round_trips_and_refuses_anything_else() {
        for g in [Generation::Current, Generation::Previous] {
            assert_eq!(Generation::from_number(g.number()), Some(g));
        }
        assert_eq!(Generation::from_number(3), None);
        assert_eq!(Generation::from_number(0), None);
    }

    #[test]
    fn abi_version_differs_because_a_loader_reads_it_before_anything_runs() {
        assert_eq!(Generation::Current.abi_version(), 2);
        assert_eq!(Generation::Previous.abi_version(), 0);
    }
}
