//! Wrap an executable, so the library can be pointed at a real file.
//!
//! Exists ahead of a proper command-line tool for one reason: the only way to know this
//! crate matches the implementation it replaces is to run both over the same input and
//! compare bytes. A library nobody can invoke cannot be checked against anything.
//!
//! ```text
//! cargo run -p selfish-container --example wrap -- <module> <out> [4|5]
//! ```

// A diagnostic probe, held to a probe's standards rather than the library's.
//
// These read structures whose layout is already known, at offsets the format fixes, and print
// what they find. Indexing, slicing and plain arithmetic over those offsets is the clearest way
// to say what is being read - a probe that wraps every field access in a fallible conversion is
// harder to check against a hex dump, which is the only thing it will ever be checked against.
// Nothing here ships: a wrong offset produces a wrong line on a terminal, not a wrong file.
//
// The library itself keeps every one of these lints. This block is the boundary between the two.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::collapsible_if,
    clippy::format_collect,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::doc_markdown,
    clippy::missing_panics_doc,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_lines
)]
use selfish_abi::Generation;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: wrap <module> <out> [4|5]");
        std::process::exit(2);
    };
    let generation = args
        .get(3)
        .and_then(|g| g.parse::<u8>().ok())
        .and_then(Generation::from_number)
        .unwrap_or(Generation::Current);

    let payload = std::fs::read(input)?;
    let container = selfish_container::build(&payload, generation)?;
    std::fs::write(output, &container)?;
    println!(
        "{output}: {} bytes from a {} byte payload, {generation}",
        container.len(),
        payload.len()
    );
    Ok(())
}
