//! Find the names in a vocabulary whose import hash is a given identifier.
//!
//! ```text
//! cargo run -p selfish-nid --example name_nid -- <nid>... --vocabulary <file>...
//! ```
//!
//! `<nid>` is `0x` and sixteen hex digits, searched in both byte orders, or the eleven
//! characters a symbol name spells. Every whitespace-separated word of every vocabulary line is
//! tried. The vocabulary belongs to whichever project mined it and is passed in by path.

// A probe reads fixed offsets and prints them, so the library's arithmetic lints do not apply.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::uninlined_format_args,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::collections::HashMap;

use selfish_nid::{ENCODED_LEN, Nid};

/// One identifier to look for, in every reading it might have been written in.
struct Wanted {
    /// As the requester wrote it.
    given: String,
    /// The readings to match against: a label and the value.
    readings: Vec<(&'static str, Nid)>,
}

fn parse(given: &str) -> Result<Wanted, String> {
    if let Some(hex) = given
        .strip_prefix("0x")
        .or_else(|| given.strip_prefix("0X"))
    {
        let value = u64::from_str_radix(hex, 16).map_err(|e| format!("{given}: {e}"))?;
        // Both byte orders, because projects print the same value in different conventions.
        return Ok(Wanted {
            given: given.to_owned(),
            readings: vec![
                ("as written", Nid::from_value(value)),
                ("byte-reversed", Nid::from_value(value.swap_bytes())),
            ],
        });
    }
    if given.chars().count() == ENCODED_LEN {
        let nid = Nid::decode(given).map_err(|e| format!("{given}: {e:?}"))?;
        return Ok(Wanted {
            given: given.to_owned(),
            readings: vec![("encoded", nid)],
        });
    }
    Err(format!(
        "{given}: not 0x + 16 hex digits, and not {ENCODED_LEN} characters"
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut wanted = Vec::new();
    let mut vocabularies = Vec::new();
    // The committed suffix unless `--suffix` tests whether the salt is what differs.
    let mut suffix = selfish_nid::suffix();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--vocabulary" {
            vocabularies.push(args.next().ok_or("--vocabulary wants a path")?);
        } else if arg == "--suffix" {
            let hex = args.next().ok_or("--suffix wants hex digits, or `none`")?;
            suffix = if hex == "none" {
                Vec::new()
            } else {
                hex_bytes(&hex)?
            };
        } else {
            wanted.push(parse(&arg)?);
        }
    }
    if wanted.is_empty() {
        return Err(
            "usage: name_nid <nid>... [--suffix <hex|none>] [--vocabulary <file>]...".into(),
        );
    }

    // Each reading's encoded form, printed always so the value can be grepped in a corpus
    // that stores encoded names.
    for want in &wanted {
        println!("{}", want.given);
        for (label, nid) in &want.readings {
            println!("  {:<14} 0x{:016x}  {}", label, nid.value(), nid.encode());
        }
    }

    if vocabularies.is_empty() {
        eprintln!("\nno vocabulary given - nothing was searched");
        return Ok(());
    }

    let mut looking: HashMap<u64, (&str, &str)> = HashMap::new();
    for want in &wanted {
        for (label, nid) in &want.readings {
            looking.insert(nid.value(), (want.given.as_str(), label));
        }
    }

    let mut found: Vec<(String, String, String, String)> = Vec::new();
    let mut words = 0_u64;
    for path in &vocabularies {
        let text = std::fs::read_to_string(path)?;
        for line in text.lines() {
            if line.starts_with('#') {
                continue;
            }
            for word in line.split_whitespace() {
                words += 1;
                let nid = Nid::with_suffix(word, &suffix);
                if let Some((given, reading)) = looking.get(&nid.value()) {
                    found.push((
                        (*given).to_owned(),
                        (*reading).to_owned(),
                        word.to_owned(),
                        path.clone(),
                    ));
                }
            }
        }
    }

    println!(
        "\n{} words hashed from {} file(s)",
        words,
        vocabularies.len()
    );
    for want in &wanted {
        let hits: Vec<_> = found.iter().filter(|f| f.0 == want.given).collect();
        if hits.is_empty() {
            println!("{}  no name in this vocabulary produces it", want.given);
        }
        for (_, reading, name, path) in hits {
            // The re-hash lets a reader verify the answer without the corpus.
            println!("{}  = {}  ({}, from {})", want.given, name, reading, path);
            println!(
                "      check: hash({:?}) = {}",
                name,
                Nid::with_suffix(name, &suffix).encode()
            );
        }
    }
    Ok(())
}

/// Sixteen hex digits, or any even number of them, as bytes.
///
/// Only for `--suffix`; the library embeds the committed suffix and takes none at run time.
fn hex_bytes(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err(format!("{hex}: an odd number of hex digits"));
    }
    (0..hex.len() / 2)
        .map(|at| {
            u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16).map_err(|e| format!("{hex}: {e}"))
        })
        .collect()
}
