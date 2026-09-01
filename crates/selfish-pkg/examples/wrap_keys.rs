//! Does this crate reproduce a real package's key blobs, byte for byte?
//!
//! Entries `0x10` and `0x20` carry the key material a console needs in order to open a
//! package's filesystem at all. Until now this crate took them as input and a caller with
//! nothing to hand supplied zeros - which produces a package that parses, extracts, and
//! passes every test here, and cannot be opened by the machine it was built for.
//!
//! # Why this is a strong check and not a round trip
//!
//! The RSA padding is drawn from a Mersenne Twister seeded from the modulus and the payload,
//! so it is **deterministic**. That means the right answer is a specific 2048 bytes, and this
//! compares against a real package's actual bytes rather than checking that what was written
//! can be read back. A round trip would pass for an implementation that is self-consistently
//! wrong; this cannot.
//!
//! `0x10` is stored in the clear, so it is compared directly. `0x20` is stored encrypted, so
//! the stored form is decrypted first and the RSA block underneath is what is compared.
//!
//! A package built with a passcode other than the fake one will differ, and that is reported
//! as such rather than as a failure - the passcode is an input nobody can recover.
//!
//! ```text
//! cargo run -p selfish-pkg --example wrap_keys -- <package>...
//! ```

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "an example reads better with plain arithmetic; the library it drives does not"
)]

use selfish_pkg::{Package, entry_id, keys};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: wrap_keys <package>...");
        std::process::exit(2);
    }

    let mut exact = 0_usize;
    let mut compared = 0_usize;
    for path in &paths {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let bytes = std::fs::read(path)?;
        let package = Package::parse(&bytes)?;
        println!("{name}");

        let content_id = package.content_id();
        // Only a package built with the fake passcode can match. Establish that first, so a
        // difference below is attributable rather than mysterious.
        let recovered = keys::filesystem_key(&package).ok();
        let computed = keys::derive_filesystem_key(content_id, keys::FAKE_PASSCODE);
        let fake_passcode = recovered.as_deref() == Some(computed.as_slice());
        if !fake_passcode {
            println!("  built with another passcode - cannot be reproduced, and should not be");
            continue;
        }
        compared += 1;

        // ---- entry 0x10, stored in the clear -------------------------------------------
        let mut all_match = true;
        let entry = package
            .entry(entry_id::ENTRY_KEYS)
            .ok_or("no entry-keys entry")?;
        let real = package.entry_bytes(entry).ok_or("truncated entry keys")?;
        let ours = keys::entry_keys_blob(content_id, keys::FAKE_PASSCODE)?;

        if real.len() < ours.len() {
            println!(
                "  0x10  real entry is {} bytes, ours {}",
                real.len(),
                ours.len()
            );
            all_match = false;
        } else {
            let real = &real[..ours.len()];
            if real == ours.as_slice() {
                println!("  0x10  {} bytes identical", ours.len());
            } else {
                all_match = false;
                let differing = real.iter().zip(&ours).filter(|(a, b)| a != b).count();
                println!("  0x10  {differing} of {} bytes differ", ours.len());
                // Which parts agree localises the fault: the header digest, the check digests
                // and the wrapped blocks fail for different reasons.
                report("        seed digest ", &real[..0x20], &ours[..0x20]);
                report(
                    "        check digests",
                    &real[0x20..0x100],
                    &ours[0x20..0x100],
                );
                for index in 0..7 {
                    let at = 0x100 + index * 0x100;
                    report(
                        &format!("        wrapped[{index}] "),
                        &real[at..at + 0x100],
                        &ours[at..at + 0x100],
                    );
                }
            }
        }

        // ---- entry 0x20, stored encrypted ----------------------------------------------
        let entry = package
            .entry(entry_id::IMAGE_KEY)
            .ok_or("no image-key entry")?;
        let real = keys::decrypt_entry(&package, entry)?;
        let ours = keys::image_key_blob(content_id, keys::FAKE_PASSCODE)?;
        if real.len() >= ours.len() && real[..ours.len()] == ours[..] {
            println!("  0x20  {} bytes identical, once decrypted", ours.len());
        } else {
            all_match = false;
            let differing = real.iter().zip(&ours).filter(|(a, b)| a != b).count();
            println!(
                "  0x20  {differing} bytes differ (real {} ours {})",
                real.len(),
                ours.len()
            );
        }

        if all_match {
            exact += 1;
            println!("  both blobs reproduced exactly");
        }
    }

    println!();
    println!("{exact} of {compared} fake-passcode package(s) reproduced exactly");
    if compared > 0 && exact < compared {
        std::process::exit(1);
    }
    Ok(())
}

/// Say whether one region matches, and how badly if not.
fn report(label: &str, real: &[u8], ours: &[u8]) {
    if real == ours {
        println!("{label} match");
    } else {
        let differing = real.iter().zip(ours).filter(|(a, b)| a != b).count();
        println!("{label} {differing}/{} differ", real.len());
    }
}
