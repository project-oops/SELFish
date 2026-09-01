//! Decrypt the entries a package marks encrypted, and say what came out.
//!
//! The two licence entries looked like unbreakable noise for as long as this crate read three
//! fields out of a 32-byte entry record and treated the other twenty as padding. They are not
//! noise: bit 31 of an entry's first flags word says it is encrypted, and bits 12-15 of the
//! second name the key. Both licence entries declare themselves, in every package examined.
//!
//! ```text
//! cargo run -p selfish-pkg --example decrypt -- <package>...
//! ```

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::naive_bytecount,
    clippy::collapsible_if,
    clippy::too_many_lines,
    reason = "an example reads better with plain arithmetic and without a dependency added to               count zero bytes once; the library it drives does neither"
)]

use sha2::{Digest as _, Sha256};

use selfish_pkg::{Package, keys};

/// What a decrypted entry looks like, as far as this can tell.
fn describe(raw: &[u8]) -> String {
    if raw.len() >= 4 && &raw[..4] == b"RIF\0" {
        return "RIF - a licence, decrypted correctly".to_owned();
    }
    let zero = raw.iter().filter(|byte| **byte == 0).count();
    let printable = raw
        .iter()
        .filter(|byte| byte.is_ascii_graphic() || **byte == b' ')
        .count();
    format!(
        "{} bytes, {}% zero, {}% printable",
        raw.len(),
        zero * 100 / raw.len().max(1),
        printable * 100 / raw.len().max(1)
    )
}

fn hex16(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: decrypt <package>...");
        std::process::exit(2);
    }

    for path in &paths {
        let bytes = std::fs::read(path)?;
        let Ok(package) = Package::parse(&bytes) else {
            println!("{path}: not a readable package");
            continue;
        };
        println!("{}", path.rsplit(['/', '\\']).next().unwrap_or(path));

        for entry in package.entries() {
            if !entry.is_encrypted() {
                continue;
            }
            print!(
                "  entry {:#07x}  key {}  {:>5} bytes  ",
                entry.id,
                entry.key_index(),
                entry.size
            );
            // What the crate actually produces, which picks the route that works.
            match keys::decrypt_entry(&package, entry) {
                Ok(plain) => print!("{}", describe(&plain)),
                Err(error) => print!("{error}"),
            }

            // The cross-check is only meaningful where the two routes are genuinely
            // independent - the key blob carries one index, and only for that one is the
            // computed route a second opinion rather than the same call again.
            if entry.key_index() == keys::IMAGE_KEY_INDEX {
                let blob = keys::decrypt_entry(&package, entry);
                let computed =
                    keys::decrypt_entry_with_passcode(&package, entry, keys::FAKE_PASSCODE);
                match (blob, computed) {
                    (Ok(a), Ok(b)) if a == b => print!("  [key blob and default passcode agree]"),
                    (Ok(_), Ok(_)) => print!("  [built with a different passcode]"),
                    _ => {}
                }
            }
            println!();

            // A licence, broken into the regions the structure defines, so "is the signature
            // actually populated" is a question with an answer rather than a guess.
            if entry.id == 0x400 {
                if let Ok(plain) = keys::decrypt_entry(&package, entry) {
                    // Optional side-channel for offline analysis of the signature.
                    if let Ok(path) = std::env::var("SELFISH_DUMP") {
                        let _ = std::fs::write(&path, &plain);
                    }
                    if plain.len() >= 0x400 && &plain[..4] == b"RIF\0" {
                        // Where the populated regions actually begin and end, rather than
                        // where a summary says they do. Two candidate layouts disagree and
                        // only one can be reconciled with the bytes.
                        let mut runs: Vec<(usize, usize)> = Vec::new();
                        for (at, byte) in plain.iter().enumerate().skip(0x60) {
                            if *byte != 0 {
                                match runs.last_mut() {
                                    Some((start, end)) if *end + 8 >= at => *end = at,
                                    _ => runs.push((at, at)),
                                }
                            }
                        }
                        print!("      populated runs:");
                        for (start, end) in &runs {
                            print!(" {:#x}..{:#x}", start, end + 1);
                        }
                        println!();

                        // Is SecretIv the digest of the padded content id?
                        let mut padded = [0_u8; 48];
                        let id = package.content_id();
                        padded[..id.len().min(48)].copy_from_slice(&id[..id.len().min(48)]);
                        let digest: [u8; 32] = Sha256::digest(padded).into();
                        for at in [0x208_usize, 0x260] {
                            if plain.len() > at + 16 {
                                println!(
                                    "      secret iv at {at:#x}: {}",
                                    if plain[at..at + 16] == digest[..16] {
                                        "== sha256(padded content id)[0..16]  CONFIRMED"
                                    } else {
                                        "does not match"
                                    }
                                );
                            }
                        }

                        // Is Secret encrypted, or is its first sixteen bytes the rest of the
                        // same digest, in the clear?
                        println!(
                            "      secret[0..16]: {}",
                            if plain[0x270..0x280] == digest[16..32] {
                                "== sha256(padded content id)[16..32] - PLAINTEXT"
                            } else {
                                "does not match the digest - encrypted"
                            }
                        );

                        // Can the committed keypair reproduce the signature? Sign the same
                        // bytes the structure signs and compare. This is the whole question of
                        // whether a licence can be produced here.
                        let signed = Sha256::digest(&plain[..0x300]);
                        println!(
                            "      signature: sha256 of the first 0x300 bytes is {}",
                            hex16(&signed)
                        );
                        println!("      stored:    {}", hex16(&plain[0x300..0x310]));

                        for (name, at, len) in [
                            ("padding", 0x6C, 468),
                            ("disc key", 0x240, 32),
                            ("secret iv", 0x260, 16),
                            ("secret", 0x270, 144),
                            ("signature", 0x300, 256),
                        ] {
                            let region = &plain[at..at + len];
                            let zero = region.iter().filter(|byte| **byte == 0).count();
                            println!(
                                "      {name:<10} {at:#05x} {len:>4} bytes  {}",
                                if zero == len {
                                    "entirely zero".to_owned()
                                } else {
                                    format!("{}% zero, populated", zero * 100 / len)
                                }
                            );
                        }
                    }
                }
            }

            if let Ok(plain) = keys::decrypt_entry(&package, entry) {
                if plain.iter().filter(|byte| **byte == 0).count() * 100 / plain.len().max(1) > 50 {
                    let text: String = plain
                        .iter()
                        .take(56)
                        .map(|byte| {
                            if byte.is_ascii_graphic() {
                                *byte as char
                            } else {
                                '.'
                            }
                        })
                        .collect();
                    println!("      {text}");
                }
            }
        }
    }
    Ok(())
}
