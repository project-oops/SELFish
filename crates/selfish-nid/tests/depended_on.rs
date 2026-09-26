//! Identifiers another repository in the collection has copied into its own source.
//!
//! A consumer with its own run-time hasher can only test the shape of its output, so the
//! values it depends on are pinned here, where a change to the hash fails first. A row is
//! added only for a value another repository has pinned.

use selfish_nid::Nid;

/// `(name, encoded, who depends on it)`.
const DEPENDED_ON: &[(&str, &str, &str)] = &[
    // oops-sdk's freestanding runtime resolves imports by name with its own hasher; its host
    // test (`tests/unit/test_freestd.c`) checks only the length of the result.
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

/// Every value another repository pinned is still what the hash produces.
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

/// Every pinned encoding decodes back to the hash of its name.
#[test]
fn every_pinned_encoding_decodes_back_to_the_hash_it_names() {
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
