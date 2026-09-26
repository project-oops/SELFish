//! Hash every path of a real package's inner filesystem with this crate's path hash, in both
//! the mount-rooted (`/eboot.bin`) and image-rooted (`/uroot/eboot.bin`) forms, and look each
//! up in the package's own flat path table. Nothing here reads the table back otherwise.
//!
//! ```text
//! cargo run -p selfish-pkg --example fpt_hash_check -- <package>
//! ```

// A probe reads fixed offsets and prints them, so the library's arithmetic lints do not apply.
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
use std::collections::BTreeSet;

use selfish_pfs::{Compressed, Filesystem, Slice, Source, Superblock, Xts};

/// This crate's hash of a path, reached through the public entry writer: the first four bytes of
/// an entry are the hash, so nothing private has to be exposed to ask the question.
fn path_hash(path: &str) -> u32 {
    let entry = selfish_pfs::write::path_table_entry(path, 0, false);
    u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]])
}

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let real = std::env::args()
        .nth(1)
        .expect("usage: fpt_hash_check <real.pkg>");
    let image = inner(&std::fs::read(&real)?)?;
    let source = Slice::new(&image, 0);
    let fs = Filesystem::new(&source)?;

    // Every hash the real table holds, as a set to look paths up in.
    let table = fs.contents(1)?;
    let mut present: BTreeSet<u32> = BTreeSet::new();
    for pair in table.as_chunks::<8>().0 {
        present.insert(u32::from_le_bytes([pair[0], pair[1], pair[2], pair[3]]));
    }
    println!("real table: {} entries", present.len());

    let paths: Vec<String> = fs.walk(0)?.into_iter().map(|f| f.path).collect();
    println!("tree:       {} paths\n", paths.len());

    // Two conventions, and only one of them can be what the vendor's tool hashed.
    for (name, strip) in [
        ("as walked (/uroot/...)", false),
        ("rooted at the mount", true),
    ] {
        let mut hit = 0;
        let mut miss_examples = Vec::new();
        let mut tried = 0;
        for path in &paths {
            if path == "/flat_path_table" {
                continue;
            }
            let candidate = if strip {
                path.strip_prefix("/uroot").unwrap_or(path).to_owned()
            } else {
                path.clone()
            };
            tried += 1;
            if present.contains(&path_hash(&candidate)) {
                hit += 1;
            } else if miss_examples.len() < 3 {
                miss_examples.push(candidate);
            }
        }
        println!(
            "{name}: {hit}/{tried} of the real tree's paths hash to an entry in its own table"
        );
        if hit != tried && !miss_examples.is_empty() {
            println!("   misses, for example: {}", miss_examples.join(", "));
        }
    }
    Ok(())
}
