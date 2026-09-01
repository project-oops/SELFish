//! Read a real package's outer container and report what it holds.
//!
//! The oracle path. Structure comes from cited open-source readers; a real package confirms
//! or refutes it, and this is how that check is run. Packages are never committed here - they
//! are pointed at.
//!
//! ```text
//! cargo run -p selfish-pkg --example inspect -- <package>...
//! ```

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
