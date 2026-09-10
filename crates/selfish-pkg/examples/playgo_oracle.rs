//! Check a built package's entry `0x1001` against a reference copy of the same entry.
//!
//! This is the check that settled D099: run against obSCEne's `playgo-chunk.dat`, built from the
//! same `LibOrbisPkg` derivation in Python, 415 of 416 bytes matched. The one that did not was
//! the inner size, which their script hardcodes and this crate measures.
//!
//! ```text
//! cargo run -p selfish-pkg --example playgo_oracle -- some.pkg reference/playgo-chunk.dat
//! ```
//!
//! The reference is optional: with one argument this just reports the two sizes, which is enough
//! to see whether the nesting is coherent - inner inside outer inside package.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "an example reads better with plain arithmetic; the library it drives does not"
)]

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: playgo_oracle <package.pkg> [reference-playgo-chunk.dat]");
        std::process::exit(2);
    };
    let bytes = std::fs::read(&path).expect("read the package");
    let package = selfish_pkg::Package::parse(&bytes).expect("parse the package");
    let entry = package
        .entry(selfish_pkg::derive::entry::PLAYGO_CHUNK_DAT)
        .expect("the package carries no entry 0x1001");
    let body = package
        .entry_bytes(entry)
        .expect("entry 0x1001 has no body");

    let at = |offset: usize| -> u64 {
        let mut value = [0_u8; 8];
        value.copy_from_slice(&body[offset..offset + 8]);
        u64::from_le_bytes(value)
    };
    println!("entry 0x1001: {} bytes", body.len());
    println!("  package     {:>12}  0x{:X}", at(0x148), at(0x148));
    println!("  inner image {:>12}  0x{:X}", at(0x158), at(0x158));

    let Some(reference) = args.next() else { return };
    let reference = std::fs::read(&reference).expect("read the reference");
    let shared = reference.len().min(body.len());
    let differing: Vec<usize> = (0..shared)
        .filter(|at| reference[*at] != body[*at])
        .collect();
    if differing.is_empty() && reference.len() == body.len() {
        println!("  identical to the reference, byte for byte");
        return;
    }
    println!(
        "  {} of {} bytes differ from the reference:",
        differing.len(),
        body.len()
    );
    for at in differing.iter().take(32) {
        println!(
            "    {at:#05x}: reference {:02x}, built {:02x}",
            reference[*at], body[*at]
        );
    }
}
