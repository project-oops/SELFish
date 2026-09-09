//! Identifiers another project has written into its own source, pinned here.
//!
//! `known_pairs` proves the algorithm against 389 pairs produced elsewhere. This proves
//! something narrower and, for a collection of projects sharing one hash, just as necessary:
//! that the specific values a *sibling* has copied out of here still come out the same.
//!
//! The difference matters because of how the copy is used. A consumer that resolves symbols
//! at run time carries its own hasher, and its own test can only check the shape of what that
//! hasher returns - eleven characters, every time, whatever the salt or the alphabet. So the
//! consumer's test cannot fail on a wrong constant; only a value from the format authority
//! can, and only if that value is written down somewhere it will be run.
//!
//! Somewhere it will be run is here. A line in this table is a promise to another repository:
//! change the hash and this fails before their build does.
//!
//! Adding a row is for a value another project has actually pinned - not for every name
//! anybody asks about. The answer to a question is a line in an inbox; a row here is a
//! standing commitment, and a fixture that collects everything stops saying anything.

use selfish_nid::Nid;

/// `(name, encoded, who asked and why)`.
const DEPENDED_ON: &[(&str, &str, &str)] = &[
    // oops-sdk's freestanding runtime resolves imports by name at run time and carries its
    // own hasher to do it. Its host test (`tests/unit/test_freestd.c`) checks only that the
    // result is eleven characters long, so a wrong salt or a wrong alphabet passes there and
    // fails on the console as "every import missing", which looks like a broken loader.
    // Requested as REQ-20260909T1244Z-716c.
    (
        "sceKernelGetProcessId",
        "ciYaJofC6tg",
        "oops-sdk: runtime resolver",
    ),
    (
        "sceKernelWrite",
        "4wSze92BhLI",
        "oops-sdk: runtime resolver",
    ),
    (
        "scePadReadState",
        "YndgXqQVV7c",
        "oops-sdk: runtime resolver",
    ),
];

#[test]
fn every_value_a_sibling_pinned_still_holds() {
    let wrong: Vec<String> = DEPENDED_ON
        .iter()
        .filter_map(|(name, expected, who)| {
            let ours = Nid::of(name).encode();
            (ours != *expected)
                .then(|| format!("{name} ({who}): pinned {expected}, produced {ours}"))
        })
        .collect();

    assert!(
        wrong.is_empty(),
        "{} pinned value(s) changed - a sibling repository is now wrong:\n  {}",
        wrong.len(),
        wrong.join("\n  ")
    );
}

#[test]
fn every_pinned_encoding_decodes_back_to_the_hash_it_names() {
    // The consumers resolve in both directions: hash a name to find an export, and read an
    // encoded name out of a symbol table to see what an import wants. A pin that only holds
    // one way round would let the second use drift silently.
    for (name, encoded, who) in DEPENDED_ON {
        let decoded = Nid::decode(encoded)
            .unwrap_or_else(|e| panic!("{name} ({who}): {encoded} does not decode: {e:?}"));
        assert_eq!(
            decoded,
            Nid::of(name),
            "{name} ({who}): {encoded} decodes to another hash"
        );
    }
}
