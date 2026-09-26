//! Relocations: the entries that turn a linked image into a placed one.
//!
//! Entries are standard `Elf64_Rela`; the table offsets are relative to the vendor segment.
//! `DT_RELA` holds data relocations and `DT_JMPREL` the procedure linkage table, one slot per
//! imported function. The two are applied differently and are kept apart.
//!
//! Applying relocations needs a base address and a missing-symbol policy, which belong to
//! the consumer; this module reports what each entry asks for.

use zerocopy::{FromBytes, Immutable, KnownLayout, little_endian};

/// Size of one `Elf64_Rela`.
pub const RELA_SIZE: usize = 24;

/// Relocation types, as the x86-64 ABI numbers them.
///
/// Only the types these modules use, plus the TLS types so a consumer can recognise and
/// refuse them rather than skip them.
pub mod kind {
    /// Nothing to do.
    pub const NONE: u32 = 0;
    /// Write `symbol + addend`.
    pub const ABS64: u32 = 1;
    /// Copy an object's bytes from the module that defines it.
    pub const COPY: u32 = 5;
    /// Write `symbol` - a data symbol's address, in a global offset table slot.
    pub const GLOB_DAT: u32 = 6;
    /// Write `symbol` - a function address, in a procedure linkage table slot.
    pub const JUMP_SLOT: u32 = 7;
    /// Write `base + addend` - an internal pointer adjusted for placement.
    pub const RELATIVE: u32 = 8;
    /// Thread-local: the module id owning the variable.
    pub const DTPMOD64: u32 = 16;
    /// Thread-local: the offset of the variable within that module's block.
    pub const DTPOFF64: u32 = 17;
    /// Thread-local: the offset of the variable from the thread pointer.
    pub const TPOFF64: u32 = 18;
    /// The address is produced by calling a resolver inside the image itself.
    pub const IRELATIVE: u32 = 37;

    /// The ABI's name for a type, where this module knows one.
    ///
    /// `None` for a type this module does not list.
    #[must_use]
    pub const fn name(kind: u32) -> Option<&'static str> {
        Some(match kind {
            NONE => "NONE",
            ABS64 => "64",
            COPY => "COPY",
            GLOB_DAT => "GLOB_DAT",
            JUMP_SLOT => "JUMP_SLOT",
            RELATIVE => "RELATIVE",
            DTPMOD64 => "DTPMOD64",
            DTPOFF64 => "DTPOFF64",
            TPOFF64 => "TPOFF64",
            IRELATIVE => "IRELATIVE",
            _ => return None,
        })
    }
}

/// One relocation entry, exactly as it appears on disk.
#[derive(Debug, Clone, Copy, FromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct Rela {
    /// Where to write, as a virtual address before placement.
    pub offset: little_endian::U64,
    /// Packed symbol index and relocation type.
    pub info: little_endian::U64,
    /// Constant added to the computed value.
    pub addend: little_endian::I64,
}

impl Rela {
    /// The relocation type, from the low half of `info`.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the low half is the field, so the truncation is the definition"
    )]
    pub fn kind(&self) -> u32 {
        (self.info.get() & 0xFFFF_FFFF) as u32
    }

    /// Index into the dynamic symbol table, from the high half.
    ///
    /// Zero is a valid index, so [`Self::needs_symbol`] decides whether it is meaningful.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the high half is the field"
    )]
    pub fn symbol_index(&self) -> u32 {
        (self.info.get() >> 32) as u32
    }

    /// Whether this type resolves against a symbol.
    #[must_use]
    pub fn needs_symbol(&self) -> bool {
        matches!(
            self.kind(),
            kind::ABS64 | kind::COPY | kind::GLOB_DAT | kind::JUMP_SLOT
        )
    }

    /// Whether this type needs thread-local storage to exist first.
    #[must_use]
    pub fn is_tls(&self) -> bool {
        matches!(self.kind(), kind::DTPMOD64 | kind::DTPOFF64 | kind::TPOFF64)
    }
}

/// Read a relocation table out of a byte range.
///
/// A trailing partial entry is dropped, so a rounded length in the dynamic table does not
/// make the image unreadable.
#[must_use]
pub fn table(bytes: &[u8]) -> Vec<Rela> {
    bytes
        .as_chunks::<RELA_SIZE>()
        .0
        .iter()
        .filter_map(|chunk| Rela::read_from_prefix(chunk).ok().map(|(entry, _)| entry))
        .collect()
}

/// A count of each relocation type present, most common first.
///
/// What the file contains; what a consumer supports is the consumer's concern.
#[must_use]
pub fn census(entries: &[Rela]) -> Vec<(u32, usize)> {
    let mut out: Vec<(u32, usize)> = Vec::new();
    for entry in entries {
        let kind = entry.kind();
        match out.iter_mut().find(|(seen, _)| *seen == kind) {
            Some((_, count)) => *count = count.saturating_add(1),
            None => out.push((kind, 1)),
        }
    }
    out.sort_unstable_by_key(|(kind, count)| (core::cmp::Reverse(*count), *kind));
    out
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
    use super::{RELA_SIZE, census, kind, table};

    fn entry(offset: u64, symbol: u32, kind: u32, addend: i64) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(&((u64::from(symbol) << 32) | u64::from(kind)).to_le_bytes());
        out.extend_from_slice(&addend.to_le_bytes());
        out
    }

    /// `info` holds the type in the low half and the symbol index in the high half.
    #[test]
    fn info_splits_into_a_symbol_index_and_a_type() {
        let bytes = entry(0x1000, 42, kind::JUMP_SLOT, 0);
        let read = table(&bytes);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].kind(), kind::JUMP_SLOT);
        assert_eq!(read[0].symbol_index(), 42);
        assert_eq!(read[0].offset.get(), 0x1000);
    }

    /// A `RELATIVE` entry needs no symbol and keeps a negative addend.
    #[test]
    fn a_relative_relocation_needs_no_symbol_and_carries_a_signed_addend() {
        let bytes = entry(0x2000, 0, kind::RELATIVE, -8);
        let read = table(&bytes);
        assert!(!read[0].needs_symbol());
        assert_eq!(read[0].addend.get(), -8, "signed, and negative ones occur");
    }

    /// A symbol-bound type needs a symbol even at index zero.
    #[test]
    fn symbol_index_zero_is_not_the_test_for_needing_a_symbol() {
        let bytes = entry(0x3000, 0, kind::GLOB_DAT, 0);
        assert!(table(&bytes)[0].needs_symbol());
    }

    /// A trailing partial entry is dropped and the whole ones are kept.
    #[test]
    fn a_trailing_partial_entry_is_dropped_rather_than_refused() {
        let mut bytes = entry(0x1000, 1, kind::ABS64, 0);
        bytes.extend_from_slice(&[0; RELA_SIZE - 1]);
        assert_eq!(table(&bytes).len(), 1);
    }

    /// The TLS types report `is_tls` and `RELATIVE` does not.
    #[test]
    fn tls_types_are_recognised_rather_than_left_to_look_ordinary() {
        for tls in [kind::DTPMOD64, kind::DTPOFF64, kind::TPOFF64] {
            let bytes = entry(0, 0, tls, 0);
            assert!(table(&bytes)[0].is_tls(), "{tls} should be TLS");
        }
        assert!(!table(&entry(0, 0, kind::RELATIVE, 0))[0].is_tls());
    }

    /// An unlisted type has no name.
    #[test]
    fn an_unknown_type_has_no_name_rather_than_a_plausible_one() {
        assert_eq!(kind::name(kind::JUMP_SLOT), Some("JUMP_SLOT"));
        assert_eq!(kind::name(0xDEAD), None);
    }

    /// The census lists the most common type first.
    #[test]
    fn the_census_is_ordered_by_count() {
        let mut bytes = Vec::new();
        for _ in 0..3 {
            bytes.extend_from_slice(&entry(0, 0, kind::RELATIVE, 0));
        }
        bytes.extend_from_slice(&entry(0, 0, kind::JUMP_SLOT, 0));
        assert_eq!(
            census(&table(&bytes)),
            vec![(kind::RELATIVE, 3), (kind::JUMP_SLOT, 1)]
        );
    }
}
