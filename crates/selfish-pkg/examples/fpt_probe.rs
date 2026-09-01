//! What the *inner* filesystem's flat path table actually looks like, ours against a real one.
//!
//! A console reads this table to find files inside `/app0`. This crate writes the layout
//! `LibOrbisPkg` describes - a previous-generation tool - and `LibProsperoPKG@main` keeps a
//! **separate** current-generation table (`ProsperoPs5FlatPathTable`) beside the older one,
//! described as a `0x40`-byte header with a `7F 46 4C 54` magic at `+0x20` and sixteen-byte
//! entries of `(path hash, packed payload)` sorted by hash. Its own note says that format is for
//! the "nwonly inner-image" path and is distinct from the legacy hashmap-style one.
//!
//! Two documented formats and no statement of which a real package here uses is exactly the
//! situation to settle by looking, rather than by picking the one that sounds current. The inner
//! filesystem is plain, so no key is needed past the one that opens the outer image.
//!
//!     fpt_probe <ours.pkg> <real.pkg>
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

/// The magic a current-generation table carries at `+0x20`, per `LibProsperoPKG@main`.
const PS5_MAGIC: [u8; 4] = [0x7F, b'F', b'L', b'T'];

/// The inner filesystem image, decrypted and decompressed out of a package.
fn inner(pkg: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
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
    Ok(src.read(0, u64::from_le_bytes(len) as usize)?)
}

fn dump(label: &str, image: &[u8]) {
    println!("\n===== {label} =====");
    let source = Slice::new(image, 0);
    let fs = match Filesystem::new(&source) {
        Ok(fs) => fs,
        Err(err) => {
            println!("  the inner filesystem will not open: {err:?}");
            return;
        }
    };
    // The flat path table is inode 1 of an inner filesystem, before anything a caller supplied.
    let table = match fs.contents(1) {
        Ok(bytes) => bytes,
        Err(err) => {
            println!("  no flat path table: {err:?}");
            return;
        }
    };
    println!("  flat path table: {} bytes", table.len());

    let magic_at_20 = table.get(0x20..0x24).is_some_and(|m| m == PS5_MAGIC);
    println!(
        "  current-generation magic (7F 'F' 'L' 'T') at +0x20: {}",
        if magic_at_20 { "YES" } else { "no" }
    );
    if magic_at_20 {
        let u32at =
            |o: usize| u32::from_le_bytes([table[o], table[o + 1], table[o + 2], table[o + 3]]);
        println!("    version      {}", u32at(0x00));
        println!("    entry stride {:#x}", table[0x04]);
        println!("    data starts  {:#x}", u32at(0x08));
        println!("    entry count  {}", u32at(0x2C));
    }

    let head: String = table
        .iter()
        .take(64)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .chunks(16)
        .map(|c| c.join(""))
        .collect::<Vec<_>>()
        .join("\n                  ");
    println!("  first 64 bytes:  {head}");

    // What the filesystem actually holds, so the table can be read against the tree it describes.
    let files: Vec<String> = fs
        .walk(0)
        .unwrap_or_default()
        .into_iter()
        .map(|f| f.path)
        .collect();
    println!("  tree ({} entries): {}", files.len(), files.join(", "));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ours = std::env::args()
        .nth(1)
        .expect("usage: fpt_probe <ours> <real>");
    let real = std::env::args()
        .nth(2)
        .expect("usage: fpt_probe <ours> <real>");
    dump("OURS", &inner(&std::fs::read(&ours)?)?);
    dump("REAL", &inner(&std::fs::read(&real)?)?);
    Ok(())
}
