//! Why a console opens `pfs_image.dat` from one package's outer mount and not another's.
//!
//! A console mounts the outer image at `<title>-app0-nest` and opens `pfs_image.dat` from the
//! root of that mount. On a package this crate built the open fails with `ENOENT`; on a real
//! one it succeeds. Both have the same tree (`/flat_path_table`, `/uroot/pfs_image.dat`), so the
//! difference is in the bytes: an inode field or a dirent the console reads and this crate's own
//! reader is lenient about.
//!
//! This decrypts two packages and dumps the raw inode table and the raw directory/path-table
//! blocks side by side, so the differing field is visible.
//!
//!     nest_diff <ours.pkg> <real.pkg>
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

const INODE_SIZE: usize = 0x2C8;

fn decrypt(bytes: &[u8]) -> Result<(Superblock, Vec<u8>), Box<dyn std::error::Error>> {
    let package = selfish_pkg::Package::parse(bytes)?;
    let key = selfish_pkg::keys::filesystem_key(&package)?;
    let at = package.image_offset()?;
    let image = Slice::new(bytes, at);
    let sb_raw = image.read(0, 0x400)?;
    let sb = Superblock::parse(&sb_raw)?;
    let block_size = sb.block_size as u64;
    let (tweak, data) = selfish_pfs::image_keys(&key, &sb_raw)?;
    let sectors = block_size / selfish_pfs::SECTOR_SIZE;
    let xts = Xts::new(image, &tweak, &data, sectors)?;
    let len = (sb.data_blocks.max(sb.inode_blocks + 8) * block_size) as usize;
    let plain = xts.read(0, len.min(bytes.len()))?;
    Ok((sb, plain))
}

/// A compact view of one inode: the fields that matter for a mount.
fn inode_line(raw: &[u8]) -> String {
    let u16at = |o: usize| u16::from_le_bytes([raw[o], raw[o + 1]]);
    let u32at = |o: usize| u32::from_le_bytes([raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]);
    let u64at = |o: usize| {
        u64::from_le_bytes([
            raw[o],
            raw[o + 1],
            raw[o + 2],
            raw[o + 3],
            raw[o + 4],
            raw[o + 5],
            raw[o + 6],
            raw[o + 7],
        ])
    };
    format!(
        "mode={:#06x} nlink={} flags={:#010x} size={} sizec={} \
         t0={:#x} t1={:#x} blocks={} start={:#x}",
        u16at(0x00),
        u16at(0x02),
        u32at(0x04),
        u64at(0x08),
        u64at(0x10),
        u64at(0x18),
        u64at(0x20),
        u32at(0x60),
        u32at(0x64),
    )
}

fn dump(label: &str, sb: &Superblock, img: &[u8]) {
    let block = sb.block_size as usize;
    println!("\n===== {label} =====");
    println!(
        "  superblock: block_size={:#x} inode_count={} inode_blocks={} data_blocks={}",
        sb.block_size, sb.inode_count, sb.inode_blocks, sb.data_blocks
    );
    // The inode table starts at block 1.
    let table = block;
    println!("  inodes (from block 1):");
    for i in 0..sb.inode_count as usize {
        let at = table + i * INODE_SIZE;
        if at + INODE_SIZE > img.len() {
            break;
        }
        println!("    [{i}] {}", inode_line(&img[at..]));
    }
    // Each inode's `start` names its first data block; dump the non-zero head of each, which for
    // a directory is its dirents and for the path table is its entries.
    for i in 0..sb.inode_count as usize {
        let at = table + i * INODE_SIZE;
        if at + 0x68 > img.len() {
            break;
        }
        let start = u32::from_le_bytes([
            img[at + 0x64],
            img[at + 0x65],
            img[at + 0x66],
            img[at + 0x67],
        ]) as usize;
        let boff = start * block;
        if boff == 0 || boff + 64 > img.len() {
            continue;
        }
        // First 64 bytes of the block this inode points at.
        let head = &img[boff..boff + 64];
        let hex: String = head.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = head
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("    inode[{i}] block {start:#x}: {hex}");
        println!("                       \"{ascii}\"");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ours = std::env::args()
        .nth(1)
        .expect("usage: nest_diff <ours> <real>");
    let real = std::env::args()
        .nth(2)
        .expect("usage: nest_diff <ours> <real>");
    let ob = std::fs::read(&ours)?;
    let rb = std::fs::read(&real)?;
    let (osb, oi) = decrypt(&ob)?;
    let (rsb, ri) = decrypt(&rb)?;
    dump("OURS", &osb, &oi);
    dump("REAL", &rsb, &ri);
    Ok(())
}
