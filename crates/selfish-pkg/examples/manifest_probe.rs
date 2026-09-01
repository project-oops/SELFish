//! Find what the three unfilled manifest digests are digests *of*.
//!
//! A package's manifest (entry `0x80`) holds SHA-256 digests every `0x20` bytes. This crate
//! fills `0x40` (the image) and `0xC0` (`param.sfo`), and a real package also fills `0x20`,
//! `0x60` and `0xA0` - which nothing here can currently compute, so a package it builds is
//! refused by a console's installer (`m_is_pkg = false`).
//!
//! Matching those three against every *stored* region of a package finds nothing, so they are
//! digests of the **plaintext** the package does not carry: the decrypted outer image, the
//! `PFSC` container inside it, and the inner filesystem image. This decrypts a real package
//! through exactly the chain `selfish extract` uses and hashes each layer, so the answer comes
//! from a real file rather than a guess.
//!
//!     manifest_probe <package>
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
use selfish_pfs::{Compressed, Filesystem, Slice, Source, Xts};
use sha2::{Digest, Sha256};

fn sha(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: manifest_probe <package>");
    let bytes = std::fs::read(&path)?;
    let package = selfish_pkg::Package::parse(&bytes)?;

    // The manifest's own digests, so the comparison is against this very file.
    let manifest = package
        .entry(selfish_pkg::derive::entry::MANIFEST)
        .and_then(|e| package.entry_bytes(e))
        .expect("no manifest entry");
    let want: Vec<(usize, String)> = [0x20_usize, 0x40, 0x60, 0x80, 0xA0, 0xC0]
        .iter()
        .map(|at| {
            let slot = manifest.get(*at..at + 32).unwrap_or(&[]);
            (*at, slot.iter().map(|b| format!("{b:02x}")).collect())
        })
        .collect();
    for (at, d) in &want {
        println!("manifest[{at:#04x}] = {d}");
    }

    // The decrypt chain, exactly as `extract` does it.
    let key = selfish_pkg::keys::filesystem_key(&package)?;
    let at = package.image_offset()?;
    let image = Slice::new(&bytes, at);
    let superblock = image.read(0, 0x400)?;
    let block_size = u64::from(u32::from_le_bytes([
        *superblock.get(0x20).unwrap_or(&0),
        *superblock.get(0x21).unwrap_or(&0),
        *superblock.get(0x22).unwrap_or(&0),
        *superblock.get(0x23).unwrap_or(&0),
    ]));
    let (tweak, data) = selfish_pfs::image_keys(&key, &superblock)?;
    let sectors = block_size / selfish_pfs::SECTOR_SIZE;
    let decrypted = Xts::new(image, &tweak, &data, sectors)?;

    let stored_len = u64::try_from(bytes.len()).unwrap_or(0).saturating_sub(at);
    println!("\nimage at {at:#x}, {stored_len} bytes to end, block size {block_size:#x}");

    let mut candidates: Vec<(String, String)> = Vec::new();
    let mut add = |name: String, digest: String| candidates.push((name, digest));

    // The stored (encrypted) image as the package holds it.
    add(
        "stored image (encrypted, to end of file)".to_owned(),
        sha(bytes.get(usize::try_from(at).unwrap_or(0)..).unwrap_or(&[])),
    );

    // The decrypted outer image, whole and at candidate lengths: the layer boundary is exactly
    // what is being looked for, so several plausible extents are hashed rather than one guess.
    if let Ok(plain) = decrypted.read(0, usize::try_from(stored_len).unwrap_or(0)) {
        add("decrypted outer image (to end)".to_owned(), sha(&plain));
        for len in [
            0x10000_usize,
            0x100000,
            0x400000,
            usize::try_from(block_size).unwrap_or(0),
        ] {
            if len > 0 && len <= plain.len() {
                add(
                    format!("decrypted outer, first {len:#x}"),
                    sha(&plain[..len]),
                );
            }
        }
    }

    // The outer filesystem holds exactly one file: the PFSC container. Inside that is the inner
    // image. Neither is stored in the package, which is what makes them candidates.
    if let Ok(outer) = Filesystem::new(&decrypted) {
        for found in outer.walk(0).unwrap_or_default() {
            let Ok(contents) = outer.contents(found.inode) else {
                continue;
            };
            add(
                format!(
                    "outer file {} ({} bytes, the PFSC)",
                    found.path,
                    contents.len()
                ),
                sha(&contents),
            );
            if contents.len() >= 0x30 {
                let dl = u64::from_le_bytes(contents[0x28..0x30].try_into().unwrap());
                println!("PFSC data_length (inner pfs size) = {dl} (0x{dl:x})");
            }

            // Through the PFSC decompressor to the inner image.
            let slice = Slice::new(&contents, 0);
            if let Ok(inner_src) = Compressed::new(slice) {
                for len in [
                    contents.len(),
                    0x10000,
                    0x100000,
                    0x400000,
                    0x1000000,
                    0x4000000,
                ] {
                    if let Ok(inner) = inner_src.read(0, len) {
                        add(
                            format!(
                                "inner image (decompressed, {len:#x} requested -> {} bytes)",
                                inner.len()
                            ),
                            sha(&inner),
                        );
                    }
                }
            }
        }
    }

    println!("\ncandidates:");
    for (name, digest) in &candidates {
        let hit = want
            .iter()
            .find(|(_, d)| d == digest)
            .map(|(at, _)| format!("   <<< MATCHES manifest[{at:#04x}]"))
            .unwrap_or_default();
        println!("  {digest}  {name}{hit}");
    }
    Ok(())
}
