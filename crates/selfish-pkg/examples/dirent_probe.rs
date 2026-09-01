//! Walk the inner filesystem's directory blocks the way a consumer must, ours against a real one.
//!
//! A directory block is a run of variable-length entries, each declaring its own size, and the run
//! ends when an entry declares zero. Anything reading it steps by that declared size, so a wrong
//! one does not produce a wrong answer - it produces a walk that lands mid-entry and keeps going.
//! That is the shape of fault that takes a kernel down rather than returning an error, and it is
//! the last structure in the inner image that has not been compared against real material.
//!
//! This crate's own reader is not evidence here: it reads what it writes, and it reads real
//! packages, so it is lenient about exactly the thing in question. So this steps the block by the
//! declared sizes, reports where each entry lands, and says whether the run terminates inside the
//! block or runs off the end.
//!
//!     dirent_probe <ours.pkg> <real.pkg>
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
use selfish_pfs::{Compressed, Filesystem, Slice, Source, Superblock, Xts};

/// A plain inode, which is what an inner filesystem uses.
const INODE_SIZE: usize = 0xA8;
/// Where an inode records its first data block.
const PLAIN_START: usize = 0x64;

fn inner(pkg: &[u8]) -> Result<(u64, Vec<u8>), Box<dyn std::error::Error>> {
    let package = selfish_pkg::Package::parse(pkg)?;
    let key = selfish_pkg::keys::filesystem_key(&package)?;
    let at = package.image_offset()?;
    let image = Slice::new(pkg, at);
    let sb_raw = image.read(0, 0x400)?;
    let sb = Superblock::parse(&sb_raw)?;
    let block = sb.block_size as u64;
    let (tweak, data) = selfish_pfs::image_keys(&key, &sb_raw)?;
    let xts = Xts::new(image, &tweak, &data, block / selfish_pfs::SECTOR_SIZE)?;
    let outer = Filesystem::new(&xts)?;
    let mut pfsc = Vec::new();
    for found in outer.walk(0)? {
        if found.path.ends_with(selfish_pfs::outer::IMAGE_NAME) {
            pfsc = outer.contents(found.inode)?;
        }
    }
    let mut len = [0_u8; 8];
    len.copy_from_slice(&pfsc[0x28..0x30]);
    let src = Compressed::new(Slice::new(&pfsc, 0))?;
    Ok((block, src.read(0, u64::from_le_bytes(len) as usize)?))
}

/// Step one directory block entry by entry, exactly as a consumer stepping by declared sizes must.
fn walk_block(image: &[u8], base: usize, block: usize, label: &str) {
    let mut at = 0_usize;
    let mut count = 0_usize;
    loop {
        if at + 16 > block {
            println!(
                "    {label}: ran to the end of the block with no terminator ({count} entries) <<< BAD"
            );
            return;
        }
        let u32at = |o: usize| {
            u32::from_le_bytes([
                image[base + o],
                image[base + o + 1],
                image[base + o + 2],
                image[base + o + 3],
            ])
        };
        let inode = u32at(at);
        let kind = u32at(at + 4);
        let name_len = u32at(at + 8) as usize;
        let size = u32at(at + 12) as usize;

        if size == 0 {
            println!(
                "    {label}: terminated cleanly after {count} entries, at {at:#x} of {block:#x}"
            );
            return;
        }
        if !size.is_multiple_of(8) {
            println!(
                "    {label}: entry {count} declares size {size}, not a multiple of 8 <<< BAD"
            );
            return;
        }
        if 16 + name_len > size {
            println!(
                "    {label}: entry {count} name ({name_len}) does not fit its size ({size}) <<< BAD"
            );
            return;
        }
        let name = String::from_utf8_lossy(&image[base + at + 16..base + at + 16 + name_len]);
        if count < 6 {
            println!("      [{count}] inode={inode} kind={kind} size={size} name={name:?}");
        }
        at += size;
        count += 1;
        if count > 4096 {
            println!("    {label}: more than 4096 entries, giving up <<< BAD");
            return;
        }
    }
}

fn dump(label: &str, block: u64, image: &[u8]) {
    println!("\n===== {label} =====");
    let block = block as usize;
    let table = block;
    // The super root is inode 0 and the mount root inode 2; both are directories.
    for (number, what) in [(0_usize, "super root"), (2, "uroot")] {
        let inode = table + number * INODE_SIZE;
        let start = u32::from_le_bytes([
            image[inode + PLAIN_START],
            image[inode + PLAIN_START + 1],
            image[inode + PLAIN_START + 2],
            image[inode + PLAIN_START + 3],
        ]) as usize;
        println!("  {what} (inode {number}) data block {start:#x}:");
        if start * block + block > image.len() {
            println!("    block is past the end of the image <<< BAD");
            continue;
        }
        walk_block(image, start * block, block, what);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ours = std::env::args()
        .nth(1)
        .expect("usage: dirent_probe <ours> <real>");
    let real = std::env::args()
        .nth(2)
        .expect("usage: dirent_probe <ours> <real>");
    let (ob, oi) = inner(&std::fs::read(&ours)?)?;
    let (rb, ri) = inner(&std::fs::read(&real)?)?;
    dump("OURS", ob, &oi);
    dump("REAL", rb, &ri);
    Ok(())
}
