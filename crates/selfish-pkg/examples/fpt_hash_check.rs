//! Does this crate's path hash agree with a real package's flat path table?
//!
//! `write::flat_path_table` says the risk out loud: nothing here reads the table back - a reader
//! follows directory entries, which are the authority - so a wrong hash passes every test in this
//! repository and fails only on the console. It has never been checked against real material.
//!
//! This checks it the only way that settles it. A real package's inner filesystem carries both the
//! table *and* the tree it describes, so every path can be hashed with this crate's function and
//! looked up in the table the vendor's own tool wrote. Agreement is the hash confirmed against
//! somebody else's output; disagreement is a console being handed a lookup structure it cannot
//! use, which is a candidate for the panic that happens while mounting `/app0`.
//!
//! The path convention is checked too, because the table stores a hash and not a string: this
//! crate writes paths rooted at the mount (`/eboot.bin`) while a walk reports them rooted at the
//! image (`/uroot/eboot.bin`), and only one of those can be what was hashed.
//!
//!     fpt_hash_check <real.pkg>
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
