//! Name a hash, the other way round: given an identifier, find a name that produces it.
//!
//! Every other use of this crate runs forwards - hash a name you already have and ask a
//! table where it lives. An import a running guest calls that nothing can name is the
//! reverse question, and it has no closed-form answer: the hash is one-way, so the only
//! method is to hash a vocabulary and look for the value.
//!
//! That makes the vocabulary the whole of the answer's strength, and the vocabulary is
//! **not this repository's**. It is passed in by path - a mining product belongs to
//! whichever project mined it (the admission test in `CLAUDE.md`), and a search tool that
//! carried its own word list would quietly become a second copy of somebody else's corpus.
//!
//! # Byte order is the first thing to get wrong
//!
//! An identifier read one way and the same identifier read the other way are both plausible
//! sixteen-hex-digit numbers, and two projects printing the same import disagree about which
//! they mean. So a raw value is searched **both ways**, and the output says which one hit.
//! An eleven-character identifier has no such ambiguity and is taken as it stands.
//!
//! ```text
//! cargo run -p selfish-nid --example name_nid -- <nid>... --vocabulary <file>...
//! ```
//!
//! `<nid>` is `0x` and sixteen hex digits, or the eleven characters a symbol name spells.
//! A vocabulary file is read a line at a time and **every** whitespace-separated word on the
//! line is tried, so a table with the name in any column works without being reshaped first.

// A diagnostic probe, held to a probe's standards rather than the library's. See the note in
// `selfish-container`'s examples: nothing here ships, and a wrong line on a terminal is the
// worst it can produce.
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
        // Both readings, because two projects print the same import differently and neither
        // is wrong about its own convention.
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
    // The committed suffix unless told otherwise. Varying it is an experiment the crate
    // already provides for: an identifier that no name explains is one of the few reasons to
    // ask whether the salt itself is what differs, and asking is cheaper than speculating.
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

    // The forward direction first: what each reading looks like as a symbol name spells it.
    // Printed whether or not a vocabulary is supplied, because it is what makes the value
    // greppable in a corpus that stores the encoded form rather than the raw one.
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
            // Re-hashing the name is the point: the answer verifies itself, and a reader
            // holding only the name can reproduce it without the corpus it came from.
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
/// Only here for `--suffix`. The library parses the committed one itself and does not take a
/// suffix from anywhere a caller could reach at run time - that is the point of D004.
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
