//! Read a real package's outer container and report what it holds.
//!
//! The oracle path. Structure comes from cited open-source readers; a real package confirms
//! or refutes it, and this is how that check is run. Packages are never committed here - they
//! are pointed at.
//!
//! ```text
//! cargo run -p selfish-pkg --example inspect -- <package>...
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
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::expect_used,
    clippy::unwrap_used
)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: inspect <package>...");
        std::process::exit(2);
    }

    for path in &paths {
        let bytes = std::fs::read(path)?;
        let name = std::path::Path::new(path)
            .file_name()
            .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());

        match selfish_pkg::Package::parse(&bytes) {
            Ok(package) => {
                let missing = package.missing_expected_entries();
                println!(
                    "{name}: {} entries, {} of the expected set missing",
                    package.entries().len(),
                    missing.len()
                );
                if !missing.is_empty() {
                    println!(
                        "  missing: {}",
                        missing
                            .iter()
                            .map(|id| format!("{id:#x}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                match selfish_pkg::keys::filesystem_key(&package) {
                    Ok(key) => println!(
                        "  filesystem key: {} bytes, begins {}",
                        key.len(),
                        key.iter().take(8).fold(String::new(), |mut acc, b| {
                            use core::fmt::Write as _;
                            let _ = write!(acc, "{b:02x}");
                            acc
                        })
                    ),
                    Err(error) => println!("  filesystem key: {error}"),
                }
                let extra: Vec<String> = package
                    .entries()
                    .iter()
                    .filter(|e| !selfish_pkg::entry_id::ALWAYS_PRESENT.contains(&e.id))
                    .map(|e| format!("{:#x}", e.id))
                    .collect();
                if !extra.is_empty() {
                    println!("  beyond the expected set: {}", extra.join(", "));
                }
            }
            Err(error) => println!("{name}: {error}"),
        }
    }
    Ok(())
}
