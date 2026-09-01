//! Relocations: the entries that turn a linked image into a placed one.
//!
//! Standard `Elf64_Rela` throughout - the vendor adds nothing to the entry format. What it
//! does add is *where* the tables live: like every other dynamic table in these modules, the
//! offsets are relative to the vendor segment rather than to the file.
//!
//! # Two tables, and the split is the interesting part
//!
//! - **`DT_RELA`** holds data relocations - absolute addresses baked into the image that must
//!   be adjusted for wherever it actually landed.
//! - **`DT_JMPREL`** holds the procedure linkage table, one slot per imported function.
//!   **This is where an import becomes a call**: whatever address goes in a slot is what runs.
//!
//! Keeping them apart matters because they are applied differently and at different times. A
//! reader that concatenates them produces a correct-looking list no consumer can act on.
//!
//! # What this module does not do
//!
//! It does not apply them. Computing `symbol + addend` needs a base address and a policy for
//! what to do when a symbol is missing - consumer concerns, and the three consumers answer
//! them differently. This module says what is in the table and what each entry asks for.

use zerocopy::{FromBytes, Immutable, KnownLayout, little_endian};

/// Size of one `Elf64_Rela`.
pub const RELA_SIZE: usize = 24;

/// Relocation types, as the x86-64 ABI numbers them.
///
/// Not every type is listed - only those that turn up in these modules, plus the TLS ones,
/// which are listed precisely so they can be *recognised and refused* rather than skipped. A
/// skipped relocation leaves a pointer that looks valid and is not.
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
    /// `None` rather than a made-up string: an unrecognised type in one of these tables is a
    /// thing to go and look up, and a plausible label is how it stops being noticed.
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
    /// Meaningless for types that need no symbol, where it is conventionally zero - and zero
    /// is also a valid index, so [`Self::needs_symbol`] is the test rather than this being
    /// non-zero.
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
/// A trailing partial entry is dropped rather than refused: the length comes from the dynamic
/// table, and a rounding disagreement there should not make an otherwise-readable image
/// unreadable.
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
/// A census rather than a judgement. What a consumer *supports* is the consumer's business;
/// what a file *contains* is this crate's.
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

    #[test]
    fn info_splits_into_a_symbol_index_and_a_type() {
        // The halves are the opposite way round from the obvious reading - the *type* is
        // low - and getting it backwards produces symbol indices in the thousands and types
        // that are all zero, which reads as an image with nothing to relocate.
        let bytes = entry(0x1000, 42, kind::JUMP_SLOT, 0);
        let read = table(&bytes);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].kind(), kind::JUMP_SLOT);
        assert_eq!(read[0].symbol_index(), 42);
        assert_eq!(read[0].offset.get(), 0x1000);
    }

    #[test]
    fn a_relative_relocation_needs_no_symbol_and_carries_a_signed_addend() {
        let bytes = entry(0x2000, 0, kind::RELATIVE, -8);
        let read = table(&bytes);
        assert!(!read[0].needs_symbol());
        assert_eq!(read[0].addend.get(), -8, "signed, and negative ones occur");
    }

    #[test]
    fn symbol_index_zero_is_not_the_test_for_needing_a_symbol() {
        // Zero is a real index. Deciding by `symbol_index() != 0` drops every relocation
        // against the first symbol in the table.
        let bytes = entry(0x3000, 0, kind::GLOB_DAT, 0);
        assert!(table(&bytes)[0].needs_symbol());
    }

    #[test]
    fn a_trailing_partial_entry_is_dropped_rather_than_refused() {
        let mut bytes = entry(0x1000, 1, kind::ABS64, 0);
        bytes.extend_from_slice(&[0; RELA_SIZE - 1]);
        assert_eq!(table(&bytes).len(), 1);
    }

    #[test]
    fn tls_types_are_recognised_rather_than_left_to_look_ordinary() {
        for tls in [kind::DTPMOD64, kind::DTPOFF64, kind::TPOFF64] {
            let bytes = entry(0, 0, tls, 0);
            assert!(table(&bytes)[0].is_tls(), "{tls} should be TLS");
        }
        assert!(!table(&entry(0, 0, kind::RELATIVE, 0))[0].is_tls());
    }

    #[test]
    fn an_unknown_type_has_no_name_rather_than_a_plausible_one() {
        assert_eq!(kind::name(kind::JUMP_SLOT), Some("JUMP_SLOT"));
        assert_eq!(kind::name(0xDEAD), None);
    }

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
