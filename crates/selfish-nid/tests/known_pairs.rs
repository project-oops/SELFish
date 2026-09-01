//! Every name-and-encoding pair anybody else's implementation has been seen to produce.
//!
//! # Why this is the important test
//!
//! The hash is four independent choices - suffix, byte order, alphabet, bit packing - and a
//! mistake in any one yields eleven ordinary-looking characters that resolve to nothing.
//! Nothing about the output says it is wrong.
//!
//! A single published pair constrains all four, which is why one exists as a unit test. This
//! constrains them **389 times, against implementations that were not consulted while writing
//! ours**. The pairs were harvested from the resolution logs of open-source emulators, each
//! line a case where somebody else's code hashed a name, matched it, and printed what it
//! matched.
//!
//! That is what makes a single shared implementation defensible. The case for keeping two was
//! that a probe must not share a hash with the emulator it measures, or "the symbol resolved"
//! proves only that both did the same thing. True - but two of *our own* implementations
//! agreeing is evidence about us, while agreeing with 389 pairs produced elsewhere is evidence
//! about the algorithm. The fixture answers the objection better than the duplication did.
//! (D004)
//!
//! # If this fails
//!
//! Do not adjust the fixture. It is the record of what other implementations produce, and a
//! disagreement means this crate is wrong or the harvest was misread - in that order of
//! likelihood.

use selfish_nid::Nid;

/// The harvested pairs, as `<encoded> <name>` lines.
const PAIRS: &str = include_str!("known-pairs.txt");

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

    // A fixture file that silently stopped being read would make this test pass while
    // proving nothing, which is the failure mode the whole crate is written against.
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

#[test]
fn every_harvested_encoding_decodes_back_to_the_hash_of_its_name() {
    // The other direction. Encoding could be right while decoding is wrong, and a reader
    // built on this crate would then mis-resolve every import in a real module.
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
