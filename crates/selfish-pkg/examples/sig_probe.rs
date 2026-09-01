//! Confirm the header-signature recipe against a real package before relying on it.
//!
//! LibOrbisPkg writes, at `0x1000`, `RSA2048EncryptKey(PkgPublicKeys[3], SHA256(pkg[0..0x1000]))` -
//! a wrap under a *public* key, the same operation this crate already uses for the key blobs.
//! This reproduces it from a real package and compares, so "the header is signed" becomes a
//! recipe this crate can execute rather than a wall.
//!
//!     sig_probe <package>
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
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("usage: sig_probe <package>");
    let bytes = std::fs::read(&path)?;

    let hash = Sha256::digest(&bytes[..0x1000]);
    let modulus = selfish_pkg::keys::pkg_public_modulus(3).expect("pkg_public_3 unreadable");
    let sig = selfish_pkg::wrap::wrap_key(&modulus, &hash)?;

    let stored = &bytes[0x1000..0x1100];
    let hx = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    println!("computed sig[0..16] = {}", hx(&sig[..16]));
    println!("stored   sig[0..16] = {}", hx(&stored[..16]));
    println!("MATCH = {}", sig.as_slice() == stored);
    Ok(())
}
