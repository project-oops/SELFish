//! Compare the outer filesystem's **indirect signature block** against a real package's.
//!
//! A file inode holds twelve block signatures inline. Past twelve, the format spills into an
//! indirect block, and the inode's thirteenth slot signs that block instead of a data block.
//! This crate builds that layout and it has never been checked against a real one.
//!
//! # What this was written to check, and what it found: nothing wrong
//!
//! Which packages took a console down correlated exactly with whether they used an indirect
//! block - the twelve-block one survived, the 28- and 171-block ones panicked - and a real
//! package with 944 blocks mounts, so the format was clearly fine and the writer was the suspect.
//!
//! **The two are the same file.** Same twelve inline block numbers, same thirteenth slot naming
//! the same indirect block, same `36`-byte stride inside it, `blocks - 12` entries in both, zeroes
//! after:
//!
//! ```text
//!                     inode slots        indirect at   entries   after
//!   ours   171 blocks  0x7..0x12 + 0x5   0x5           159       zeroes
//!   real   944 blocks  0x7..0x12 + 0x5   0x5           932       zeroes
//! ```
//!
//! **The correlation was an artefact of when each package failed.** The twelve-block package did
//! not survive because it avoided the indirect block - it survived because it was refused at the
//! cache check (D070), which happens before whatever panics, so it never reached the code in
//! question. A package that fails earlier for an unrelated reason is not a control; it is a
//! package that was never tested, and reading it as one nearly bought a rewrite of a correct
//! block writer.
//!
//! Kept because the comparison is worth being able to re-run, and because the answer *no
//! difference* is one somebody will otherwise spend a day finding again.
//!
//!     indirect_probe <ours.pkg> <real.pkg>
// A diagnostic probe, held to a probe's standards rather than the library's.
//
// These read structures whose layout is already known, at offsets the format fixes, and print
// what they find. Indexing, slicing and plain arithmetic over those offsets is the clearest way
// to say what is being read - a probe that wraps every field access in a fallible conversion is
// harder to check against a hex dump, which is the only thing it will ever be checked against.
// Nothing here ships: a wrong offset produces a wrong line on a terminal, not a wrong package.
//
// The library itself keeps every one of these lints. This block is the boundary between the two.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::format_collect,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::doc_markdown,
    clippy::missing_panics_doc,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_lines
)]
use selfish_pfs::{Slice, Source, Superblock, Xts};

/// A signed inode.
const INODE_SIZE: usize = 0x2C8;
/// Where an inode's block signatures begin.
const INODE_SIG_AT: usize = 0x64;
/// A digest and the block number it covers.
const SIG_SIZE: usize = 36;
/// How many signatures live in the inode before the format spills into an indirect block.
const DIRECT: usize = 12;
/// The file - the inner image - is always inode 3 of an outer filesystem.
const FILE_INODE: usize = 3;

fn decrypt(bytes: &[u8]) -> Result<(Superblock, Vec<u8>), Box<dyn std::error::Error>> {
    let package = selfish_pkg::Package::parse(bytes)?;
    let key = selfish_pkg::keys::filesystem_key(&package)?;
    let at = package.image_offset()?;
    let image = Slice::new(bytes, at);
    let sb_raw = image.read(0, 0x400)?;
    let sb = Superblock::parse(&sb_raw)?;
    let block = sb.block_size as u64;
    let (tweak, data) = selfish_pfs::image_keys(&key, &sb_raw)?;
    let xts = Xts::new(image, &tweak, &data, block / selfish_pfs::SECTOR_SIZE)?;
    let len = ((bytes.len() as u64) - at) as usize;
    Ok((sb, xts.read(0, len)?))
}

/// One signature slot: the digest, and the block number that follows it.
fn slot(img: &[u8], at: usize) -> (bool, u32) {
    let digest = &img[at..at + 32];
    let number = u32::from_le_bytes([img[at + 32], img[at + 33], img[at + 34], img[at + 35]]);
    (digest.iter().any(|b| *b != 0), number)
}

fn dump(label: &str, sb: &Superblock, img: &[u8]) {
    let block = sb.block_size as usize;
    let inode = block + FILE_INODE * INODE_SIZE;
    let blocks = u32::from_le_bytes([
        img[inode + 0x60],
        img[inode + 0x61],
        img[inode + 0x62],
        img[inode + 0x63],
    ]);
    println!("\n===== {label} =====");
    println!(
        "  file inode: {blocks} blocks, so indirect is {} used",
        if blocks as usize > DIRECT { "" } else { "NOT" }
    );

    // The thirteen slots the inode itself carries: twelve data blocks, then the indirect.
    println!("  inode slots (digest present? / block number):");
    for i in 0..=DIRECT {
        let (has, number) = slot(img, inode + INODE_SIG_AT + i * SIG_SIZE);
        let what = if i == DIRECT {
            "  <- the indirect block"
        } else {
            ""
        };
        println!(
            "    [{i:2}] digest={} block={number:#x}{what}",
            if has { "yes" } else { " NO" }
        );
    }

    // The indirect block itself, as the inode's thirteenth slot names it.
    let (_, indirect) = slot(img, inode + INODE_SIG_AT + DIRECT * SIG_SIZE);
    if indirect == 0 {
        println!("  no indirect block named");
        return;
    }
    let base = indirect as usize * block;
    if base + block > img.len() {
        println!("  indirect block {indirect:#x} is past the end of the image");
        return;
    }
    println!("  indirect block at {indirect:#x}, first entries:");
    for i in 0..8 {
        let at = base + i * SIG_SIZE;
        let (has, number) = slot(img, at);
        println!(
            "    [{i:2}] digest={} block={number:#x}",
            if has { "yes" } else { " NO" }
        );
    }
    // How far into the block the entries actually run: the first all-zero slot ends them.
    let mut used = 0;
    for i in 0..(block / SIG_SIZE) {
        let (has, number) = slot(img, base + i * SIG_SIZE);
        if !has && number == 0 {
            break;
        }
        used += 1;
    }
    println!(
        "  entries in use: {used} (block holds {} at {SIG_SIZE}-byte stride)",
        block / SIG_SIZE
    );
    let tail: String = img[base + used * SIG_SIZE..base + used * SIG_SIZE + 16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    println!("  first 16 bytes past the last entry: {tail}");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ours = std::env::args()
        .nth(1)
        .expect("usage: indirect_probe <ours> <real>");
    let real = std::env::args()
        .nth(2)
        .expect("usage: indirect_probe <ours> <real>");
    let (osb, oi) = decrypt(&std::fs::read(&ours)?)?;
    let (rsb, ri) = decrypt(&std::fs::read(&real)?)?;
    dump("OURS", &osb, &oi);
    dump("REAL", &rsb, &ri);
    Ok(())
}
