//! Re-deriving what a package's entries mean, from packages, and building those entries.
//!
//! Entry meanings marked `DERIVED` in `data/pkg-format.tsv` were established from real
//! packages rather than a cited source. [`run`] re-tests each such claim against any packages
//! supplied and reports how many samples it held for. Each claim is a falsifiable hypothesis;
//! no bytes are copied out of a sample.

use sha2::{Digest, Sha256};

use crate::{ENTRY_SIZE, Entry, Package};

/// Size of a SHA-256 digest, which is the slot width both derived tables use.
pub const DIGEST: usize = 32;

/// One claim about an entry, and how it fared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Which entry the claim is about.
    pub entry: u32,
    /// What is claimed, in one line.
    pub claim: &'static str,
    /// How many packages the claim held for.
    pub held: usize,
    /// How many it was testable on.
    pub tested: usize,
    /// Anything a reader needs in order to disbelieve it.
    pub notes: Vec<String>,
}

impl Finding {
    /// Whether every sample it could be tested on agreed.
    #[must_use]
    pub const fn survived(&self) -> bool {
        self.tested > 0 && self.held == self.tested
    }
}

/// What a run established.
#[derive(Debug, Clone, Default)]
pub struct Derivation {
    /// Every claim, in the order they were tested.
    pub findings: Vec<Finding>,
    /// How many packages were readable.
    pub samples: usize,
}

impl Derivation {
    /// Whether every claim survived every sample it was testable on.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        !self.findings.is_empty() && self.findings.iter().all(Finding::survived)
    }
}

/// Re-derive the entry meanings from packages.
///
/// Every claim is tested against every package given. A claim that fails on any sample is
/// reported as failing; there is no majority vote.
#[must_use]
pub fn run(packages: &[Package<'_>]) -> Derivation {
    // One finding per `DERIVED` row in `data/pkg-format.tsv`.
    let findings = vec![
        digest_table_finding(packages),
        entry_table_copy_finding(packages),
        manifest_finding(packages),
        playgo_finding(packages),
    ];

    Derivation {
        findings,
        samples: packages.len(),
    }
}

/// Entry `0x1`: one SHA-256 per entry, in table order, with its own slot zeroed.
fn digest_table_finding(packages: &[Package<'_>]) -> Finding {
    let mut held = 0_usize;
    let mut tested = 0_usize;
    let mut notes = Vec::new();

    for package in packages {
        let Some(table) = package
            .entry(entry::DIGESTS)
            .and_then(|entry| package.entry_bytes(entry))
        else {
            continue;
        };
        tested = tested.saturating_add(1);
        let listed = package.entries();

        if table.len() != listed.len().saturating_mul(DIGEST) {
            notes.push(format!(
                "a package has {} entries but a {}-byte table, so it is not one slot each",
                listed.len(),
                table.len()
            ));
            continue;
        }

        let mut wrong = 0_usize;
        for (slot, entry) in listed.iter().enumerate() {
            let at = slot.saturating_mul(DIGEST);
            let Some(chunk) = table.get(at..at.saturating_add(DIGEST)) else {
                wrong = wrong.saturating_add(1);
                continue;
            };
            // Its own slot cannot hold its own digest, and every package examined zeroes it.
            if entry.id == entry::DIGESTS {
                if chunk.iter().any(|byte| *byte != 0) {
                    wrong = wrong.saturating_add(1);
                    notes.push("the self-slot is not zero in a package".to_owned());
                }
                continue;
            }
            let Some(raw) = package.entry_bytes(entry) else {
                continue;
            };
            if chunk != sha256(raw) {
                wrong = wrong.saturating_add(1);
            }
        }
        if wrong == 0 {
            held = held.saturating_add(1);
        } else {
            notes.push(format!("{wrong} slot(s) disagreed in a package"));
        }
    }

    Finding {
        entry: entry::DIGESTS,
        claim: "one SHA-256 per entry, in table order, own slot zeroed",
        held,
        tested,
        notes,
    }
}

/// Entry `0x100`: the package entry table again, record for record.
fn entry_table_copy_finding(packages: &[Package<'_>]) -> Finding {
    let mut held = 0_usize;
    let mut tested = 0_usize;
    let mut notes = Vec::new();

    for package in packages {
        let Some(table) = package
            .entry(entry::TABLE_COPY)
            .and_then(|entry| package.entry_bytes(entry))
        else {
            continue;
        };
        tested = tested.saturating_add(1);
        let listed = package.entries();

        if table.len() != listed.len().saturating_mul(ENTRY_SIZE) {
            notes.push(format!(
                "a package has {} entries but a {}-byte copy",
                listed.len(),
                table.len()
            ));
            continue;
        }

        let mut wrong = 0_usize;
        for (slot, entry) in listed.iter().enumerate() {
            let at = slot.saturating_mul(ENTRY_SIZE);
            let Some(record) = table.get(at..at.saturating_add(ENTRY_SIZE)) else {
                wrong = wrong.saturating_add(1);
                continue;
            };
            // The same three fields at the same offsets the outer table uses.
            let found = Entry::from_row(record);
            if found.map(|f| (f.id, f.offset, f.size)) != Some((entry.id, entry.offset, entry.size))
            {
                wrong = wrong.saturating_add(1);
            }
        }
        if wrong == 0 {
            held = held.saturating_add(1);
        } else {
            notes.push(format!("{wrong} record(s) disagreed in a package"));
        }
    }

    Finding {
        entry: entry::TABLE_COPY,
        claim: "the entry table again: id at 0x00, offset at 0x10, size at 0x14, big-endian",
        held,
        tested,
        notes,
    }
}

/// Entry `0x80`: a fixed table whose slots digest named things rather than entries.
///
/// Six of its twelve slots are non-zero and two are checked, so the finding is marked partial.
fn manifest_finding(packages: &[Package<'_>]) -> Finding {
    let mut held = 0_usize;
    let mut tested = 0_usize;
    let mut notes = Vec::new();

    for package in packages {
        let Some(table) = package
            .entry(entry::MANIFEST)
            .and_then(|entry| package.entry_bytes(entry))
        else {
            continue;
        };
        tested = tested.saturating_add(1);

        let mut wrong = 0_usize;
        // The leading slot is not a digest: every package examined opens with the same four
        // bytes, recorded as observed and not interpreted.
        if table.get(..4) != Some(&manifest::LEADING) {
            wrong = wrong.saturating_add(1);
            notes.push("the leading bytes are not the ones every sample carries".to_owned());
        }

        // The image, whole, from where the header says it starts to the end of the file.
        match (
            package.image_offset(),
            table.get(manifest::IMAGE_DIGEST..manifest::IMAGE_DIGEST + DIGEST),
        ) {
            (Ok(at), Some(slot)) => {
                match usize::try_from(at)
                    .ok()
                    .and_then(|at| package.bytes().get(at..))
                {
                    Some(image) if slot == sha256(image) => {}
                    Some(_) => {
                        wrong = wrong.saturating_add(1);
                        notes.push("the image digest slot does not match the image".to_owned());
                    }
                    None => notes.push("the image runs past the end of the package".to_owned()),
                }
            }
            _ => wrong = wrong.saturating_add(1),
        }

        // The title metadata entry, digested again here as well as in the entry table.
        match (
            package
                .entry(crate::entry_id::PARAM_SFO)
                .and_then(|entry| package.entry_bytes(entry)),
            table.get(manifest::PARAM_SFO_DIGEST..manifest::PARAM_SFO_DIGEST + DIGEST),
        ) {
            (Some(raw), Some(slot)) if slot == sha256(raw) => {}
            (Some(_), Some(_)) => {
                wrong = wrong.saturating_add(1);
                notes.push("the param.sfo digest slot does not match".to_owned());
            }
            _ => {}
        }

        if wrong == 0 {
            held = held.saturating_add(1);
        }
    }

    notes.push(
        "partial: slots at 0x20, 0x60 and 0xa0 are digests of something not in the package"
            .to_owned(),
    );

    Finding {
        entry: entry::MANIFEST,
        claim: "digests of named things: the image at 0x40, param.sfo at 0xc0",
        held,
        tested,
        notes,
    }
}

/// Offsets within entry `0x80`, as far as they are established.
pub mod manifest {
    /// The four bytes every package examined opens this entry with.
    ///
    /// Recorded as observed, not interpreted. It is not a digest: it is identical in every
    /// sample while every digest slot differs.
    pub const LEADING: [u8; 4] = [0xD2, 0x56, 0x01, 0x00];
    /// A fixed word at `0x1C`, `0x6E` in every package examined.
    ///
    /// Recorded as observed, not interpreted, like [`LEADING`]. The hardware refuses a package
    /// with zero here, so the builder writes it.
    pub const FIXED_1C: usize = 0x1C;
    /// The value [`FIXED_1C`] holds, identical in the three packages measured.
    pub const FIXED_1C_VALUE: u32 = 0x6E;
    /// The content digest, `ContentDigest` in `LibOrbisPkg`. Slot for enum bit `ContentDigest`.
    pub const CONTENT_DIGEST: usize = 0x20;
    /// SHA-256 of the whole filesystem image.
    pub const IMAGE_DIGEST: usize = 0x40;
    /// The header digest, `HeaderDigest` in the general digests. Filled once the header exists.
    pub const HEADER_DIGEST: usize = 0x60;
    /// The major-param digest, SHA-256 of the major-param string.
    pub const MAJOR_PARAM_DIGEST: usize = 0xA0;
    /// SHA-256 of the `param.sfo` entry.
    pub const PARAM_SFO_DIGEST: usize = 0xC0;
}

/// How much of a package one `PLAYGO_CHUNK_SHA` slot covers.
pub const PLAYGO_BLOCK: usize = 0x10000;
/// How many bytes each slot holds - the leading bytes of the block's digest.
pub const PLAYGO_SLOT: usize = 4;

/// Entry `0x1002`: four bytes of SHA-256 per 64 KiB block of the whole package.
///
/// Only the blocks from the image onward are checked; the slots before it are zero.
fn playgo_finding(packages: &[Package<'_>]) -> Finding {
    let mut held = 0_usize;
    let mut tested = 0_usize;
    let mut notes = Vec::new();

    for package in packages {
        let Some(table) = package
            .entry(entry::PLAYGO_CHUNK_SHA)
            .and_then(|entry| package.entry_bytes(entry))
        else {
            continue;
        };
        tested = tested.saturating_add(1);
        let bytes = package.bytes();
        let blocks = bytes.len().checked_div(PLAYGO_BLOCK).unwrap_or(0);

        if table.len() != blocks.saturating_mul(PLAYGO_SLOT) {
            notes.push(format!(
                "a package is {} blocks but the table is {} bytes",
                blocks,
                table.len()
            ));
            continue;
        }

        let first = package
            .image_offset()
            .ok()
            .and_then(|at| usize::try_from(at).ok())
            .and_then(|at| at.checked_div(PLAYGO_BLOCK))
            .unwrap_or(0);

        let mut wrong = 0_usize;
        for block in first..blocks {
            let at = block.saturating_mul(PLAYGO_BLOCK);
            let Some(chunk) = bytes.get(at..at.saturating_add(PLAYGO_BLOCK)) else {
                continue;
            };
            let slot = block.saturating_mul(PLAYGO_SLOT);
            if table.get(slot..slot.saturating_add(PLAYGO_SLOT)) != sha256(chunk).get(..PLAYGO_SLOT)
            {
                wrong = wrong.saturating_add(1);
            }
        }
        if wrong == 0 {
            held = held.saturating_add(1);
        } else {
            notes.push(format!("{wrong} block(s) disagreed from the image onward"));
        }
    }

    Finding {
        entry: entry::PLAYGO_CHUNK_SHA,
        claim: "four bytes of SHA-256 per 64 KiB block, checked from the image onward",
        held,
        tested,
        notes,
    }
}

/// Build entry `0x1001`, the playgo chunk descriptor.
///
/// A fixed 416-byte structure: the magic `plgo`, a fixed header, `0xFF` filler, and the content
/// id at `0x40`. Byte-identical in all three packages examined apart from that id. The
/// hardware refuses a package without it with `0x80f00101`.
#[must_use]
pub fn playgo_chunk(content_id: &str) -> Vec<u8> {
    /// What every sample holds before the content id.
    const LEADING: [u8; 0x18] = [
        0x70, 0x6C, 0x67, 0x6F, // "plgo"
        0x00, 0x00, 0x00, 0x00, //
        0x01, 0x00, 0x01, 0x00, //
        0x01, 0x00, 0x01, 0x00, //
        0xA0, 0x01, 0x00, 0x00, //
        0x00, 0x00, 0x01, 0x00, //
    ];
    /// How long the whole entry is, in every sample.
    const LEN: usize = 416;
    /// Where the content id sits.
    const CONTENT_ID_AT: usize = 0x40;
    /// The filler run before it, which is `0xFF` rather than zero.
    const FILLER: std::ops::Range<usize> = 0x20..0x40;

    let mut out = vec![0_u8; LEN];
    if let Some(slot) = out.get_mut(..LEADING.len()) {
        slot.copy_from_slice(&LEADING);
    }
    if let Some(slot) = out.get_mut(FILLER) {
        slot.fill(0xFF);
    }
    let id = content_id.as_bytes();
    if let Some(slot) = out.get_mut(CONTENT_ID_AT..CONTENT_ID_AT.saturating_add(id.len())) {
        slot.copy_from_slice(id);
    }
    out
}

/// Build entry `0x1002` over a package buffer.
///
/// Every 64 KiB block, four bytes of digest each. A trailing partial block is not covered,
/// which is what the sizes in every sample say: the table is exactly `len / 0x10000 * 4`.
#[must_use]
pub fn playgo_chunk_sha(package: &[u8], image_at: usize) -> Vec<u8> {
    let blocks = package.len().checked_div(PLAYGO_BLOCK).unwrap_or(0);
    // Blocks before the image are not digested; their slots are zero. With `image_at` at
    // `0x80000`, slots 0-7 are zero and slot 8 is the first image block, as in the packages
    // measured. The hardware refuses a shifted table with `0x80f00101`.
    let skip = image_at.checked_div(PLAYGO_BLOCK).unwrap_or(0);
    let mut out = Vec::with_capacity(blocks.saturating_mul(PLAYGO_SLOT));
    for block in 0..blocks {
        if block < skip {
            out.extend_from_slice(&[0_u8; PLAYGO_SLOT]);
            continue;
        }
        let at = block.saturating_mul(PLAYGO_BLOCK);
        let chunk = package
            .get(at..at.saturating_add(PLAYGO_BLOCK))
            .unwrap_or_default();
        out.extend_from_slice(sha256(chunk).get(..PLAYGO_SLOT).unwrap_or_default());
    }
    out
}

/// The entries this module has established a meaning for.
pub mod entry {
    /// A table of digests over every entry. Derived; see [`super`].
    pub const DIGESTS: u32 = 0x1;
    /// A second copy of the package entry table. Derived; see [`super`].
    pub const TABLE_COPY: u32 = 0x100;
    /// Digests of named things - the image and the title metadata. Partly derived.
    ///
    /// `GENERAL_DIGESTS` in `LibOrbisPkg@6434772`.
    pub const MANIFEST: u32 = 0x80;
    /// Four bytes of SHA-256 per 64 KiB block of the package.
    ///
    /// `PLAYGO_CHUNK_SHA` in `LibOrbisPkg@6434772`.
    pub const PLAYGO_CHUNK_SHA: u32 = 0x1002;
    /// The chunk descriptor - which chunks a title has and how big each image is.
    ///
    /// `PLAYGO_CHUNK_DAT` in `LibOrbisPkg@6434772`, whose `PlayGo/ChunkDat.cs` writes it and
    /// whose fields shadPS4's `playgo_chunk.h` names. Built by [`crate::playgo::chunk_dat`].
    pub const PLAYGO_CHUNK_DAT: u32 = 0x1001;
}

/// Build entry `0x1` for a set of entry contents.
///
/// `self_slot` is the position of entry `0x1` itself, which is zeroed because it cannot hold
/// its own digest.
#[must_use]
pub fn digest_table(contents: &[&[u8]], self_slot: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(contents.len().saturating_mul(DIGEST));
    for (slot, raw) in contents.iter().enumerate() {
        if slot == self_slot {
            out.extend_from_slice(&[0_u8; DIGEST]);
        } else {
            out.extend_from_slice(&sha256(raw));
        }
    }
    out
}

/// Build entry `0x100` from an entry table.
///
/// Rows are copied whole; the fields with no known meaning are zero, as in every package
/// examined.
#[must_use]
pub fn entry_table_copy(entries: &[Entry]) -> Vec<u8> {
    // Flags included: without them the hardware reads a licence's ciphertext as its content.
    entries.iter().flat_map(Entry::row).collect()
}

fn sha256(bytes: &[u8]) -> [u8; DIGEST] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{DIGEST, digest_table, entry_table_copy};
    use crate::{ENTRY_SIZE, Entry};

    /// The digest table zeroes its own slot and hashes every other entry.
    #[test]
    fn the_digest_table_zeroes_its_own_slot_and_hashes_the_rest() {
        let contents: Vec<&[u8]> = vec![b"first", b"second", b"third"];
        let table = digest_table(&contents, 0);

        assert_eq!(table.len(), 3 * DIGEST);
        assert!(
            table[..DIGEST].iter().all(|byte| *byte == 0),
            "the self-slot is zero"
        );
        assert!(
            table[DIGEST..].iter().any(|byte| *byte != 0),
            "and the others are not"
        );
    }

    /// The table copy holds id, offset and size big-endian, one whole row per entry.
    #[test]
    fn the_table_copy_reproduces_the_three_fields_big_endian() {
        let entries = [
            Entry {
                id: 0x1,
                name_offset: 0,
                flags1: 0,
                flags2: 0,
                offset: 0x40,
                size: 0x1c0,
            },
            Entry {
                id: 0x10,
                name_offset: 0,
                flags1: 0,
                flags2: 0,
                offset: 0x2000,
                size: 0x800,
            },
        ];
        let copy = entry_table_copy(&entries);

        assert_eq!(copy.len(), 2 * ENTRY_SIZE);
        assert_eq!(&copy[0x00..0x04], &0x1_u32.to_be_bytes());
        assert_eq!(&copy[0x10..0x14], &0x40_u32.to_be_bytes());
        assert_eq!(&copy[0x14..0x18], &0x1c0_u32.to_be_bytes());
        // The second record starts a whole entry later, not packed.
        assert_eq!(&copy[ENTRY_SIZE..ENTRY_SIZE + 4], &0x10_u32.to_be_bytes());
    }

    /// Table-copy fields with no known meaning are written as zero, never invented.
    #[test]
    fn everything_this_crate_has_no_meaning_for_is_left_zero() {
        let entries = [Entry {
            id: 0x1,
            name_offset: 0,
            flags1: 0,
            flags2: 0,
            offset: 0x40,
            size: 0x1c0,
        }];
        let copy = entry_table_copy(&entries);
        assert!(copy[0x04..0x10].iter().all(|byte| *byte == 0));
        assert!(copy[0x18..ENTRY_SIZE].iter().all(|byte| *byte == 0));
    }
}
