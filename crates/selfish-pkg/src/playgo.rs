//! `playgo-chunk.dat` - package entry `0x1001`, written from `data/pkg-format.tsv`.
//!
//! # Why this is computed rather than supplied
//!
//! It was the last entry a caller had to hand over, and the reason given was that its structure
//! had no derivation here. That was wrong twice. The structure has **two** independent citable
//! readings that agree field for field - `LibOrbisPkg`'s `PlayGo/ChunkDat.cs`, which writes it,
//! and shadPS4's `playgo_chunk.h`, which declares it - and every value in it is fixed for a
//! single-chunk title except two sizes, both of which this crate already computes.
//!
//! The one that looked like a wall is the inner size. `ChunkDat.cs` leaves it zero with the
//! comment *"must update this to inner pfs image size"*, so it was never an unknown field: it is
//! the length the `PFSC` header inside the outer filesystem records for its contents, which
//! [`crate::write::inner_image_size`] reads. (D099)
//!
//! # The table is the source, and the whole structure comes from it
//!
//! [`chunk_dat`] does not carry a single offset of its own. It walks the `playgo`, `playgo_toc`
//! and `playgo_body` rows of `data/pkg-format.tsv` and writes each one according to its own
//! `type` column, then fills the three values a row cannot hold - the content id and the two
//! sizes. A row added to the table changes the output; a row changed there changes it too.
//! That is the arrangement `selfish-container` and `selfish-title` already use, for the reason
//! `CLAUDE.md` gives: a constant that exists in two places is one that will eventually disagree
//! with itself.

/// The package format table, with its provenance header.
const FORMAT: &str = include_str!("../../../data/pkg-format.tsv");

/// The groups that make up the structure, in the order they are written.
///
/// Order does not matter for correctness - every row carries its own absolute offset - but it
/// keeps a dump of the buffer readable next to the table.
const GROUPS: [&str; 3] = ["playgo", "playgo_toc", "playgo_body"];

/// How much of the 128-byte content id field is written.
///
/// A content id is 36 characters. The rest of the field stays zero, which is what both sources
/// do.
const CONTENT_ID_LEN: usize = 36;

/// Build entry `0x1001` for a single-chunk title.
///
/// `package_size` is the finished package - `0x80000` plus the outer image. `inner_size` is the
/// inner PFS image. Both are what `ChunkDat.FromProject` names in the comments beside the zeros
/// it leaves for them.
///
/// A content id longer than the field is truncated rather than refused: the builder validates
/// the content id and owns that error, so failing again here would report it twice.
///
/// # Panics
///
/// If `data/pkg-format.tsv` has lost a row this needs, or holds one that no longer parses. That
/// is a build-time fact - the table is compiled in - and a table that changed shape while this
/// code did not is not something to recover from with a plausible zero.
#[must_use]
pub fn chunk_dat(content_id: &str, package_size: u64, inner_size: u64) -> Vec<u8> {
    let mut out = vec![0_u8; file_size()];

    for group in GROUPS {
        for row in rows(group) {
            row.write_into(&mut out);
        }
    }

    let id = content_id.as_bytes();
    let take = id.len().min(CONTENT_ID_LEN);
    if let Some(source) = id.get(..take) {
        write_slice(&mut out, offset_of("playgo", "content_id"), source);
    }

    put_le(
        &mut out,
        offset_of("playgo_body", "mchunk_size"),
        package_size,
        8,
    );
    put_le(
        &mut out,
        offset_of("playgo_body", "inner_mchunk_size"),
        inner_size,
        8,
    );

    out
}

/// How long the structure is, from the table's own `(size)` row.
///
/// # Panics
///
/// If that row is absent or is not a number.
#[must_use]
pub fn file_size() -> usize {
    let row = rows("playgo")
        .find(|row| row.field == "(size)")
        .unwrap_or_else(|| panic!("pkg-format.tsv has no playgo/(size) row"));
    let size = number(row.size).unwrap_or_else(|| panic!("playgo/(size) is not a number"));
    usize::try_from(size).unwrap_or_else(|_| panic!("playgo/(size) does not fit in a usize"))
}

/// One row of the table, as the columns this module reads.
#[derive(Clone, Copy)]
struct Row<'a> {
    field: &'a str,
    offset: &'a str,
    size: &'a str,
    kind: &'a str,
    value: &'a str,
}

impl Row<'_> {
    /// Write this row's value at this row's offset, if it has both.
    ///
    /// A row whose `value` column is `-` is one something else fills in - the content id and the
    /// two sizes - and a row with no offset is the `(size)` marker. Both are skipped rather than
    /// guessed at, which is the rule the rest of the repository applies to a field nothing
    /// establishes.
    fn write_into(self, out: &mut [u8]) {
        if self.value == "-" {
            return;
        }
        let Some(at) = number(self.offset).and_then(|at| usize::try_from(at).ok()) else {
            return;
        };
        let width = number(self.size)
            .and_then(|size| usize::try_from(size).ok())
            .unwrap_or(0);

        match self.kind {
            "u8" => put_le(out, at, self.number_value(), 1),
            "u16le" => put_le(out, at, self.number_value(), 2),
            "u32le" => put_le(out, at, self.number_value(), 4),
            "u64le" => put_le(out, at, self.number_value(), 8),
            "bytes" => self.write_bytes(out, at, width),
            "string" => write_slice(out, at, self.value.as_bytes()),
            "pair" => self.write_pair(out, at),
            _ => {}
        }
    }

    /// The `value` column as a number.
    ///
    /// # Panics
    ///
    /// If it is not one, for the reason [`chunk_dat`] gives.
    fn number_value(self) -> u64 {
        number(self.value).unwrap_or_else(|| {
            panic!(
                "pkg-format.tsv row {} has a non-numeric value {:?}",
                self.field, self.value
            )
        })
    }

    /// A `bytes` row: an exact sequence, or one byte repeated to fill the field.
    ///
    /// The fill case is `reserved`, which both sources set to 32 bytes of `0xFF` rather than
    /// leaving zero. Writing it as `ff` with a size of 32 keeps the table readable; spelling out
    /// thirty-two identical bytes would not.
    ///
    /// # Panics
    ///
    /// If the value is not hexadecimal bytes.
    fn write_bytes(self, out: &mut [u8], at: usize, width: usize) {
        let mut bytes: Vec<u8> = Vec::new();
        for text in self.value.split_whitespace() {
            let byte = u8::from_str_radix(text, 16).unwrap_or_else(|_| {
                panic!(
                    "pkg-format.tsv row {} is not hex bytes: {:?}",
                    self.field, self.value
                )
            });
            bytes.push(byte);
        }
        if let (1, true, Some(fill)) = (bytes.len(), width > 1, bytes.first().copied()) {
            bytes = vec![fill; width];
        }
        write_slice(out, at, &bytes);
    }

    /// A `pair` row: two little-endian `u32`s, an offset and a size.
    ///
    /// # Panics
    ///
    /// If the value is not two numbers.
    fn write_pair(self, out: &mut [u8], at: usize) {
        let mut parts = self.value.split_whitespace();
        let pair = (parts.next().and_then(number), parts.next().and_then(number));
        let (Some(offset), Some(size)) = pair else {
            panic!(
                "pkg-format.tsv row {} is not an offset/size pair: {:?}",
                self.field, self.value
            )
        };
        put_le(out, at, offset, 4);
        put_le(out, at.saturating_add(4), size, 4);
    }
}

/// Every row of one group, in table order.
fn rows(group: &str) -> impl Iterator<Item = Row<'static>> + '_ {
    FORMAT
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(move |line| {
            let columns: Vec<&str> = line.split('\t').collect();
            if columns.first() != Some(&group) {
                return None;
            }
            Some(Row {
                field: columns.get(1).copied().unwrap_or_default(),
                offset: columns.get(2).copied().unwrap_or_default(),
                size: columns.get(3).copied().unwrap_or_default(),
                kind: columns.get(4).copied().unwrap_or_default(),
                value: columns.get(5).copied().unwrap_or_default(),
            })
        })
}

/// One row's offset.
///
/// # Panics
///
/// If the row is absent or has no offset.
fn offset_of(group: &str, field: &str) -> usize {
    let row = rows(group)
        .find(|row| row.field == field)
        .unwrap_or_else(|| panic!("pkg-format.tsv has no row for {group}/{field}"));
    number(row.offset)
        .and_then(|at| usize::try_from(at).ok())
        .unwrap_or_else(|| panic!("{group}/{field} has no usable offset"))
}

/// A decimal or `0x`-prefixed hexadecimal number, or `None` for `-` and anything else.
fn number(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    trimmed.strip_prefix("0x").map_or_else(
        || trimmed.parse::<u64>().ok(),
        |hex| u64::from_str_radix(hex, 16).ok(),
    )
}

/// Write the low `width` bytes of `value`, little-endian.
fn put_le(out: &mut [u8], at: usize, value: u64, width: usize) {
    let bytes = value.to_le_bytes();
    if let (Some(slot), Some(source)) = (
        out.get_mut(at..at.saturating_add(width)),
        bytes.get(..width),
    ) {
        slot.copy_from_slice(source);
    }
}

/// Write a byte sequence, if it fits.
fn write_slice(out: &mut [u8], at: usize, bytes: &[u8]) {
    if let Some(slot) = out.get_mut(at..at.saturating_add(bytes.len())) {
        slot.copy_from_slice(bytes);
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
    use super::{chunk_dat, file_size};

    /// obSCEne's probe package, whose entry `0x1001` is the oracle these were checked against.
    const CONTENT_ID: &str = "UP0001-PPSA01337_00-OBSCENEPROBE0000";
    const PACKAGE_SIZE: u64 = 0x0073_0000;
    const INNER_SIZE: u64 = 0x00AA_0000;

    fn built() -> Vec<u8> {
        chunk_dat(CONTENT_ID, PACKAGE_SIZE, INNER_SIZE)
    }

    #[test]
    fn the_length_comes_from_the_table_and_is_the_one_both_sources_write() {
        assert_eq!(file_size(), 416);
        assert_eq!(built().len(), 416);
    }

    #[test]
    fn the_magic_is_bytes_rather_than_a_number() {
        // Stored as `70 6c 67 6f` rather than as shadPS4's `0x6F676C70`, because a magic held
        // as an integer and written back is a magic written backwards. (D002)
        assert_eq!(&built()[0x00..0x04], b"plgo");
    }

    #[test]
    fn the_four_counts_describe_a_single_chunk_title() {
        let out = built();
        for at in [0x08, 0x0A, 0x0C, 0x0E] {
            assert_eq!(
                u16::from_le_bytes([out[at], out[at + 1]]),
                1,
                "count at {at:#x}"
            );
        }
    }

    #[test]
    fn reserved_is_filled_with_ff_rather_than_left_zero() {
        // The one field where "left alone" and "written" differ visibly, and both sources
        // write it.
        assert_eq!(&built()[0x20..0x40], &[0xFF_u8; 32]);
    }

    #[test]
    fn the_content_id_is_written_into_a_field_four_times_its_length() {
        let out = built();
        assert_eq!(&out[0x40..0x40 + CONTENT_ID.len()], CONTENT_ID.as_bytes());
        assert!(
            out[0x40 + 36..0xC0].iter().all(|byte| *byte == 0),
            "the rest of the 128-byte field stays zero"
        );
    }

    #[test]
    fn the_sub_table_index_is_written_in_source_order_not_address_order() {
        // The last pair points at 0x150, which sits between the fourth and the fifth. Both
        // sources write it there and a reader indexes by position, so tidying it would move
        // `inner_mchunk_attrs` into whatever slot came fourth.
        let out = built();
        let pair = |slot: usize| {
            let at = 0xC0 + slot * 8;
            (
                u32::from_le_bytes(out[at..at + 4].try_into().unwrap()),
                u32::from_le_bytes(out[at + 4..at + 8].try_into().unwrap()),
            )
        };
        assert_eq!(
            (0..8).map(pair).collect::<Vec<_>>(),
            vec![
                (0x100, 32),
                (0x120, 2),
                (0x130, 9),
                (0x140, 16),
                (0x160, 32),
                (0x180, 2),
                (0x190, 12),
                (0x150, 16),
            ]
        );
    }

    #[test]
    fn both_labels_are_present_and_terminated() {
        let out = built();
        assert_eq!(&out[0x130..0x138], b"Chunk #0");
        assert_eq!(out[0x138], 0);
        assert_eq!(&out[0x190..0x19B], b"Scenario #0");
        assert_eq!(out[0x19B], 0);
    }

    #[test]
    fn the_two_sizes_are_the_only_values_a_caller_changes() {
        // Everything else is fixed for a single-chunk title, so two builds differing only in
        // their sizes must differ only in those sixteen bytes.
        let one = chunk_dat(CONTENT_ID, 0x0073_0000, 0x00AA_0000);
        let two = chunk_dat(CONTENT_ID, 0x0099_0000, 0x00BB_0000);
        let differing: Vec<usize> = (0..one.len()).filter(|at| one[*at] != two[*at]).collect();
        assert!(
            differing
                .iter()
                .all(|at| (0x148..0x150).contains(at) || (0x158..0x160).contains(at)),
            "only the two size fields may differ, got {differing:?}"
        );
    }

    #[test]
    fn the_body_carries_records_rather_than_a_header_over_zeros() {
        // The recorded failure this entry exists to avoid: a console read the counts, found no
        // records behind them, and `scePlayGoCoreGetRawContentInfo` returned 0x80f00200 after
        // the header had already passed.
        let out = built();
        assert_eq!(out[0x100], 0x80, "chunk flag");
        assert_eq!(out[0x102], 3, "req_locus");
        assert_eq!(&out[0x110..0x118], &[0xFF_u8; 8], "language mask");
        assert_eq!(out[0x160], 1, "scenario type");
        assert!(
            out[0x100..0x1A0].iter().any(|byte| *byte != 0),
            "the body is written"
        );
    }

    #[test]
    fn the_package_size_lands_where_the_table_says_and_the_inner_size_after_it() {
        let out = built();
        assert_eq!(
            u64::from_le_bytes(out[0x148..0x150].try_into().unwrap()),
            PACKAGE_SIZE
        );
        assert_eq!(
            u64::from_le_bytes(out[0x158..0x160].try_into().unwrap()),
            INNER_SIZE
        );
        // Both offsets are zero: one chunk starting at the beginning of each image.
        assert_eq!(u64::from_le_bytes(out[0x140..0x148].try_into().unwrap()), 0);
        assert_eq!(u64::from_le_bytes(out[0x150..0x158].try_into().unwrap()), 0);
    }
}
