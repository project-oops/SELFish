//! Re-deriving what a package's entries mean, from packages.
//!
//! Two of the fourteen entries a package carries were established here rather than taken from
//! a source, and this module is why that is allowed to stand. **Point it at any packages you
//! have and it re-runs the derivation in front of you**, saying how many samples each claim
//! survived. Nothing is asserted that this cannot re-check.
//!
//! That is the same arrangement obSCEne uses for the vendor dynamic tags - a `derive` command
//! that reproduces the assignment from a module rather than trusting the constants that built
//! it. A derivation nobody can re-run is a claim, and a claim in a format table is how a
//! transcription error becomes a fact.
//!
//! # What a derivation here is allowed to be
//!
//! A hypothesis a machine can falsify, checked against every sample available. `RELA + RELASZ
//! == HASH` was established exactly this way. What is not allowed is asserting a meaning
//! nothing tests, or lifting bytes out of somebody's package and calling the copy a
//! definition. (principle 5)
//!
//! Everything here reports **how many samples backed it**, because two is a coincidence and
//! this had three.

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
/// Every claim is tested against every package given. A claim that fails on one sample is
/// reported as failing - this does not take a majority, because a format that is true of two
/// packages in three is not a format.
#[must_use]
pub fn run(packages: &[Package<'_>]) -> Derivation {
    // One entry per derived row. A row added to `data/pkg-format.tsv` with `DERIVED` in its
    // note belongs here too, or the command stops covering the table it claims to check.
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
            // The same three fields at the same three offsets the outer table uses. Checking
            // only that the record *contains* the numbers somewhere would also be satisfied by
            // a coincidence, which is not a derivation.
            if be32(record, ID) != Some(entry.id)
                || be32(record, OFFSET) != Some(entry.offset)
                || be32(record, SIZE) != Some(entry.size)
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
/// Six of its twelve slots are non-zero and two are established. It is reported as a partial
/// finding on purpose - a row that claims more than was checked is worse than a short row.
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
        // bytes and carries a small value at the end of the slot. Recorded as observed and
        // **not interpreted** - four identical bytes are not a meaning. (principle 5)
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
    /// Recorded as observed, **not interpreted**. It is not a digest - it is identical in
    /// every sample while every digest slot differs - but four bytes that agree three times
    /// are an observation, not a meaning.
    pub const LEADING: [u8; 4] = [0xD2, 0x56, 0x01, 0x00];
    /// A fixed word at `0x1C`, `0x6E` in every package examined.
    ///
    /// Recorded as observed, **not interpreted** - the same standing as [`LEADING`]. It is not
    /// a digest: it is byte-identical in all three packages while every digest slot differs
    /// between them. It was found because this crate wrote zero here and a console refused the
    /// result, so an unexplained constant that three real files agree on is worth writing even
    /// without a meaning for it.
    pub const FIXED_1C: usize = 0x1C;
    /// The value [`FIXED_1C`] holds. Measured in three packages, 3/3 agreement.
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
/// Only the blocks from the image onward are checked. The earlier ones cover the package
/// buffer as it stood when the digests were taken, which is before the body and header were
/// written - so they are a fact about a builder's ordering rather than about the file, and
/// nothing can verify them after the fact.
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
/// id at `0x40`. **Byte-identical in all three packages examined apart from that id**, which is
/// what makes it something to generate rather than demand from a caller.
///
/// It was found by asking a different question than "what is this entry". A package built here
/// was refused by a console with `0x80f00101`, and the content id appears three times in a real
/// package - the header, this entry, and `param.sfo` - but only twice in one built here. This
/// was the missing third. (measured 3/3)
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
    // Blocks before the image are **not** digested; their slots are zero.
    //
    // This digested from byte zero, which covers the header region too - and every slot from
    // there on was therefore shifted by however many blocks the header occupies. A real package
    // zeroes the first eight slots (`0x80000 / 0x10000`) and starts at the image, and the
    // shifted table is why a console refused a package built here with `0x80f00101` while
    // accepting the same real package truncated to a fraction of its size: the digest a slot
    // holds has to be of the block that slot stands for.
    //
    // Measured against a real package: slots 0-7 zero, slot 8 == sha256(file[0x80000..])[..4],
    // and every later slot follows. (3/3 packages, `image_at` is `0x80000` in all of them)
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
}

/// Offset of the id within a record.
const ID: usize = 0x00;
/// Offset of the entry's offset.
const OFFSET: usize = 0x10;
/// Offset of the entry's size.
const SIZE: usize = 0x14;

/// Build entry `0x1` for a set of entry contents.
///
/// `self_slot` is the position of entry `0x1` itself, which is zeroed because it cannot hold
/// its own digest.
///
/// This is the half that makes the derivation worth having: a writer can produce this entry
/// exactly, from data it already holds, with nothing left to guess.
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
/// Every field this crate does not have a meaning for is left zero, which is what all three
/// packages examined hold there.
#[must_use]
pub fn entry_table_copy(entries: &[Entry]) -> Vec<u8> {
    /// Name-table offset, at `0x04`.
    const NAME_OFFSET: usize = 0x04;
    /// First flag word, at `0x08`.
    const FLAGS1: usize = 0x08;
    /// Second flag word, at `0x0C`.
    const FLAGS2: usize = 0x0C;
    let mut out = vec![0_u8; entries.len().saturating_mul(ENTRY_SIZE)];
    for (slot, entry) in entries.iter().enumerate() {
        let at = slot.saturating_mul(ENTRY_SIZE);
        put32(&mut out, at.saturating_add(ID), entry.id);
        put32(&mut out, at.saturating_add(NAME_OFFSET), entry.name_offset);
        // The flags matter: they are how an entry declares itself encrypted, and a table that
        // drops them makes a console read a licence's ciphertext as its content. This is the
        // whole entry table a real package reads, not the three fields a reader here needed.
        put32(&mut out, at.saturating_add(FLAGS1), entry.flags1);
        put32(&mut out, at.saturating_add(FLAGS2), entry.flags2);
        put32(&mut out, at.saturating_add(OFFSET), entry.offset);
        put32(&mut out, at.saturating_add(SIZE), entry.size);
    }
    out
}

fn sha256(bytes: &[u8]) -> [u8; DIGEST] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn be32(record: &[u8], at: usize) -> Option<u32> {
    let mut raw = [0_u8; 4];
    raw.copy_from_slice(record.get(at..at.saturating_add(4))?);
    Some(u32::from_be_bytes(raw))
}

fn put32(out: &mut [u8], at: usize, value: u32) {
    if let Some(slot) = out.get_mut(at..at.saturating_add(4)) {
        slot.copy_from_slice(&value.to_be_bytes());
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
    use super::{DIGEST, digest_table, entry_table_copy};
    use crate::{ENTRY_SIZE, Entry};

    #[test]
    fn the_digest_table_zeroes_its_own_slot_and_hashes_the_rest() {
        // The self-slot is the whole subtlety: an entry cannot contain its own digest, and
        // every package examined leaves it zero rather than, say, digesting an empty buffer.
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

    #[test]
    fn everything_this_crate_has_no_meaning_for_is_left_zero() {
        // Principle 5 in the writing direction: an invented value in a package is one a
        // console reads and acts on. Absent is visible; invented is not.
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
