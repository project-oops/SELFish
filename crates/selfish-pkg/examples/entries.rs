//! What a package writer would have to produce, and how much of it is not established.
//!
//! Fourteen entries appear in every package examined. Eight have a cited meaning; six do not,
//! and "six unknown entries" has been the whole of the answer for long enough that it deserves
//! a better one.
//!
//! This asks a sharper question of the unknown six: **do they vary between titles?** An entry
//! byte-identical across three unrelated packages is boilerplate, and boilerplate is a very
//! different obstacle from a field encoding something about the title. Both are still blocked -
//! an entry that does not vary in three samples may vary in the fourth, and copying bytes
//! out of somebody's package is derivation-from-material either way - but they are blocked to
//! different degrees and by different things.
//!
//! It characterises a gap. It does not derive a format, and nothing it prints goes into
//! `data/`. (principle 2, D012)
//!
//! ```text
//! cargo run -p selfish-pkg --example entries -- <package>...
//! ```

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "an example reads better with plain arithmetic; the library it drives does not"
)]

use std::collections::BTreeMap;

use selfish_pkg::{Package, entry_id};

/// What `data/pkg-format.tsv` records for each entry, so the report says what is settled.
fn known(id: u32) -> Option<&'static str> {
    Some(match id {
        0x10 => "entry keys",
        0x20 => "image key",
        0x200 => "filename table",
        0x409 => "zeros",
        0x1000 => "param.sfo",
        0x1001 => "playgo chunk",
        0x1003 => "xml",
        0x1200 => "png (the icon)",
        _ => return None,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.len() < 2 {
        eprintln!("usage: entries <package> <package> [package...]");
        eprintln!("at least two, because the question is whether an entry varies between them");
        std::process::exit(2);
    }

    // id -> one sample per package, so an entry can be compared across titles.
    let mut seen: BTreeMap<u32, Vec<(String, Vec<u8>)>> = BTreeMap::new();
    let mut titles = Vec::new();

    for path in &paths {
        let bytes = std::fs::read(path)?;
        let package = Package::parse(&bytes)?;
        let name = path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(path)
            .trim_end_matches(".pkg")
            .to_owned();
        titles.push(name.clone());
        for entry in package.entries() {
            if let Some(raw) = package.entry_bytes(entry) {
                seen.entry(entry.id)
                    .or_default()
                    .push((name.clone(), raw.to_vec()));
            }
        }
    }

    println!("{} packages: {}", titles.len(), titles.join(", "));
    println!();
    println!(
        "{:<8} {:>8}  {:<14} meaning",
        "entry", "bytes", "across titles"
    );

    // Counted over the entries a writer must produce, not over everything any package
    // happened to carry. An entry present in one package trivially "matches itself", and the
    // first version of this counted nine of those as boilerplate.
    let mut identical_unknown = Vec::new();
    let mut varying_unknown = Vec::new();
    let mut identified = 0;

    for (id, samples) in &seen {
        let sizes: Vec<usize> = samples.iter().map(|(_, raw)| raw.len()).collect();
        let same_size = sizes.windows(2).all(|pair| pair[0] == pair[1]);
        let same_bytes = same_size && samples.windows(2).all(|pair| pair[0].1 == pair[1].1);

        let across = if samples.len() < titles.len() {
            "not in all"
        } else if same_bytes {
            "identical"
        } else if same_size {
            "same size"
        } else {
            "varies"
        };

        let meaning = known(*id).unwrap_or("NOT ESTABLISHED");
        if entry_id::ALWAYS_PRESENT.contains(id) {
            if known(*id).is_some() {
                identified += 1;
            } else if same_bytes && samples.len() == titles.len() {
                identical_unknown.push(*id);
            } else {
                varying_unknown.push(*id);
            }
        }

        println!(
            "{:#08x} {:>8}  {across:<14} {meaning}",
            id,
            sizes.first().copied().unwrap_or(0)
        );
    }

    println!();
    println!(
        "of the {} entries a writer must produce:",
        entry_id::ALWAYS_PRESENT.len()
    );
    println!("  {identified} have a cited meaning");
    println!(
        "  {} are byte-identical across all {} titles - boilerplate{}",
        identical_unknown.len(),
        titles.len(),
        list(&identical_unknown)
    );
    println!(
        "  {} differ between titles, so each encodes something{}",
        varying_unknown.len(),
        list(&varying_unknown)
    );
    println!();
    println!("An entry that differs cannot be filled with a constant, and nothing here can say");
    println!("what it should hold. That is the block, and it is a source problem: what is");
    println!("needed is a packaging tool whose source says what goes in these. (D012, D022)");
    Ok(())
}

fn list(ids: &[u32]) -> String {
    if ids.is_empty() {
        return String::new();
    }
    let names: Vec<String> = ids.iter().map(|id| format!("{id:#x}")).collect();
    format!(": {}", names.join(", "))
}
