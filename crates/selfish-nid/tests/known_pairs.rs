//! Name-and-encoding pairs produced by independent implementations.
//!
//! The pairs come from the resolution logs of open-source emulators, each a name another
//! implementation hashed and matched. Together they constrain the suffix, byte order, alphabet
//! and bit packing, which is what lets one shared implementation serve probes and emulator
//! alike (D004).
//!
//! The fixture records what other implementations produce and is not adjusted to match this
//! crate.

use selfish_nid::Nid;

/// The harvested pairs, as `<encoded> <name>` lines.
const PAIRS: &str = include_str!("known-pairs.txt");

/// Every harvested pair's name hashes to its recorded encoding.
#[test]
fn every_harvested_pair_is_reproduced() {
    let mut checked = 0_usize;
    let mut wrong = Vec::new();

    for line in PAIRS.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((encoded, name)) = line.split_once(' ') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let ours = Nid::of(name).encode();
        if ours != encoded {
            wrong.push(format!("{name}: expected {encoded}, produced {ours}"));
        }
        checked = checked.saturating_add(1);
    }

    // Fails if the fixture is not being read at all.
    assert!(
        checked >= 380,
        "only {checked} pairs were read; the fixture is not being loaded"
    );

    assert!(
        wrong.is_empty(),
        "{} of {checked} pairs disagree:\n  {}",
        wrong.len(),
        wrong.join("\n  ")
    );
}

/// Every harvested encoding decodes back to the hash of its name.
#[test]
fn every_harvested_encoding_decodes_back_to_the_hash_of_its_name() {
    for line in PAIRS.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((encoded, name)) = line.split_once(' ') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        assert_eq!(
            Nid::decode(encoded).map(Nid::value),
            Ok(Nid::of(name).value()),
            "decoding {encoded} should give the hash of {name}"
        );
    }
}
