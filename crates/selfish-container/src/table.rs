//! Reading the format table.
//!
//! Every constant this crate uses comes from `data/self-format.tsv`, which carries in its
//! header the projects and commits each field was established from. Nothing is written twice.
//!
//! The reason is the one the charter gives for `data/` in general: a constant that exists in
//! two places is a constant that will eventually disagree with itself. That is not a
//! hypothetical here - it is why this repository exists.

/// The field table, with its provenance header.
const FORMAT: &str = include_str!("../../../data/self-format.tsv");

/// One value from the table, by group and field.
///
/// `None` rather than a default. A missing constant means the table changed shape and this
/// code did not, and continuing with a plausible zero produces a container that is wrong in
/// a way nothing downstream can detect.
#[must_use]
pub fn lookup(group: &str, field: &str) -> Option<u64> {
    for line in FORMAT.lines() {
        if line.starts_with('#') {
            continue;
        }
        let mut columns = line.split('\t');
        let (Some(g), Some(f)) = (columns.next(), columns.next()) else {
            continue;
        };
        if g != group || f != field {
            continue;
        }
        // struct, field, offset, size, type, value - the value is the sixth column.
        return parse_number(columns.nth(3)?);
    }
    None
}

/// Every row of a group, as `(field, value)`.
#[must_use]
pub fn group(group: &str) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    for line in FORMAT.lines() {
        if line.starts_with('#') {
            continue;
        }
        let mut columns = line.split('\t');
        let (Some(g), Some(field)) = (columns.next(), columns.next()) else {
            continue;
        };
        if g != group {
            continue;
        }
        if let Some(value) = columns.nth(3).and_then(parse_number) {
            out.push((field.to_owned(), value));
        }
    }
    out
}

/// The note column of a row, which is where the table records *why*.
#[must_use]
pub fn note(group: &str, field: &str) -> Option<String> {
    for line in FORMAT.lines() {
        if line.starts_with('#') {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.first() != Some(&group) || columns.get(1) != Some(&field) {
            continue;
        }
        return columns.get(6).map(|s| (*s).to_owned());
    }
    None
}

/// One row that pins a concrete value at a concrete place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedField {
    /// The field name, as the table spells it.
    pub field: String,
    /// Where it sits, in bytes from the start of its struct.
    pub offset: usize,
    /// How many bytes it occupies.
    pub size: usize,
    /// The value the table says it holds.
    pub value: u64,
    /// The note column, which records why.
    pub note: String,
}

/// Every row of a struct that pins a concrete value at a concrete offset.
///
/// This is what an audit checks a real file against: a row with a `-` in its value or offset
/// column describes a field whose value varies (a size, a count), and there is nothing to
/// confirm. A row with all three is a claim the table makes that a real file can settle.
#[must_use]
pub fn fixed_fields(group: &str) -> Vec<FixedField> {
    let mut out = Vec::new();
    for line in FORMAT.lines() {
        if line.starts_with('#') {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.first() != Some(&group) {
            continue;
        }
        let (Some(field), Some(offset), Some(size), Some(value)) = (
            columns.get(1),
            columns.get(2).and_then(|c| parse_number(c)),
            columns.get(3).and_then(|c| parse_number(c)),
            columns.get(5).and_then(|c| parse_number(c)),
        ) else {
            continue;
        };
        // A `(size)` marker row carries the struct's total length, not a field to check.
        if field.starts_with('(') {
            continue;
        }
        out.push(FixedField {
            field: (*field).to_owned(),
            offset: usize::try_from(offset).unwrap_or(usize::MAX),
            size: usize::try_from(size).unwrap_or(0),
            value,
            note: columns.get(6).map_or_else(String::new, |s| (*s).to_owned()),
        });
    }
    out
}

/// `0x`-prefixed hex or plain decimal, matching how the table writes them.
#[must_use]
pub fn parse_number(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    trimmed.strip_prefix("0x").map_or_else(
        || trimmed.parse::<u64>().ok(),
        |hex| u64::from_str_radix(hex, 16).ok(),
    )
}

#[cfg(test)]
mod tests {
    use super::{group, lookup, note, parse_number};

    #[test]
    fn numbers_parse_in_both_notations_the_table_uses() {
        assert_eq!(parse_number("0x10"), Some(16));
        assert_eq!(parse_number("16"), Some(16));
        assert_eq!(parse_number(" 0x20 "), Some(32));
        assert_eq!(parse_number("-"), None);
        assert_eq!(parse_number("nonsense"), None);
    }

    #[test]
    fn no_field_is_described_twice() {
        // A duplicate row is silent in the worst way: `lookup` takes the first, so a second
        // row carrying the real value is invisible while the table plainly contains it.
        let mut seen = std::collections::BTreeSet::new();
        for line in super::FORMAT.lines() {
            if line.starts_with('#') {
                continue;
            }
            let mut columns = line.split('\t');
            let (Some(g), Some(f)) = (columns.next(), columns.next()) else {
                continue;
            };
            if g == "struct" {
                continue;
            }
            assert!(
                seen.insert(format!("{g}/{f}")),
                "the table describes {g}/{f} more than once"
            );
        }
    }

    #[test]
    fn every_struct_size_matches_the_fields_it_declares() {
        // Offsets and the declared size are two independent statements about one layout. A
        // container built where they disagree is reported by a loader only as "not a
        // container", with no indication of which field.
        let mut ends: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
        let mut declared: std::collections::BTreeMap<String, u64> =
            std::collections::BTreeMap::new();
        for line in super::FORMAT.lines() {
            if line.starts_with('#') {
                continue;
            }
            let columns: Vec<&str> = line.split('\t').collect();
            let (Some(g), Some(f)) = (columns.first(), columns.get(1)) else {
                continue;
            };
            if *g == "struct" {
                continue;
            }
            let size = columns.get(3).and_then(|s| parse_number(s));
            if *f == "(size)" {
                if let Some(s) = size {
                    declared.insert((*g).to_owned(), s);
                }
                continue;
            }
            let (Some(offset), Some(size)) = (columns.get(2).and_then(|s| parse_number(s)), size)
            else {
                continue;
            };
            let end = offset.saturating_add(size);
            let slot = ends.entry((*g).to_owned()).or_insert(0);
            if end > *slot {
                *slot = end;
            }
        }
        for (name, want) in declared {
            let got = ends.get(&name).copied().unwrap_or(0);
            assert_eq!(got, want, "{name}: fields end at {got}, declared {want}");
        }
    }

    #[test]
    fn the_groups_the_builder_relies_on_are_present() {
        assert!(!group("segment_flag").is_empty());
        assert!(!group("entry_prop").is_empty());
        assert!(!group("phdr_type").is_empty());
        assert_eq!(lookup("ptype", "fake"), Some(1));
    }

    #[test]
    fn notes_are_readable_because_they_carry_the_reasoning() {
        // The note column is where the table records *why* a value is what it is. Losing it
        // to a parsing change would leave the numbers without their evidence.
        let note = note("self_header", "flags").unwrap_or_default();
        assert!(
            note.contains("signed_block_count"),
            "the derivation of the flags constant should survive in the table"
        );
    }
}
