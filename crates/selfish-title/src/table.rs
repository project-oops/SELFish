//! Reading `data/sfo-format.tsv`.
//!
//! The same arrangement `selfish-container` uses for its own table, and for the same reason:
//! a constant that exists in two places is a constant that will eventually disagree with
//! itself. Deliberately duplicated as a *reader* rather than shared, because sharing it would
//! mean this crate depending on the container for a `split('\t')`.

/// The field table, with its provenance header.
const FORMAT: &str = include_str!("../../../data/sfo-format.tsv");

/// One numeric value from the table, by group and field.
///
/// # Panics
///
/// If the row is absent. That is a build-time fact - the table is compiled in - and a missing
/// row means the table changed shape while this code did not. Continuing with a plausible
/// zero produces a file that is wrong in a way nothing downstream can detect.
#[must_use]
pub(crate) fn u16_at(group: &str, field: &str) -> u16 {
    let value = lookup(group, field)
        .unwrap_or_else(|| panic!("sfo-format.tsv has no row for {group}/{field}"));
    u16::try_from(value).unwrap_or_else(|_| panic!("{group}/{field} does not fit in a u16"))
}

/// The bytes of a `bytes`-typed row, written space-separated in hex.
///
/// # Panics
///
/// If the row is absent or is not a byte sequence, for the reason above.
#[must_use]
pub(crate) fn bytes_at(group: &str, field: &str) -> Vec<u8> {
    let text = column(group, field, 5)
        .unwrap_or_else(|| panic!("sfo-format.tsv has no row for {group}/{field}"));
    text.split_whitespace()
        .map(|byte| {
            u8::from_str_radix(byte, 16)
                .unwrap_or_else(|_| panic!("{group}/{field} is not hex bytes: {text:?}"))
        })
        .collect()
}

/// The `value` column, parsed as a number.
#[must_use]
pub(crate) fn lookup(group: &str, field: &str) -> Option<u64> {
    let text = column(group, field, 5)?;
    let trimmed = text.trim();
    trimmed.strip_prefix("0x").map_or_else(
        || trimmed.parse::<u64>().ok(),
        |hex| u64::from_str_radix(hex, 16).ok(),
    )
}

/// One column of one row, as text.
fn column(group: &str, field: &str, index: usize) -> Option<String> {
    for line in FORMAT.lines() {
        if line.starts_with('#') {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.first() != Some(&group) || columns.get(1) != Some(&field) {
            continue;
        }
        return columns.get(index).map(|text| (*text).to_owned());
    }
    None
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{bytes_at, lookup, u16_at};

    #[test]
    fn the_magic_is_read_as_bytes_rather_than_as_a_number() {
        // The one mistake this crate inherited a warning about: a magic stored as an integer
        // and serialised back is a magic written backwards. (D002)
        assert_eq!(bytes_at("header", "magic"), vec![0x00, 0x50, 0x53, 0x46]);
    }

    #[test]
    fn the_version_is_four_bytes_and_not_a_number() {
        assert_eq!(bytes_at("header", "version"), vec![0x01, 0x01, 0x00, 0x00]);
    }

    #[test]
    fn the_format_codes_come_from_the_table() {
        assert_eq!(u16_at("format", "utf8"), 0x0204);
        assert_eq!(u16_at("format", "utf8_special"), 0x0004);
        assert_eq!(u16_at("format", "integer"), 0x0404);
    }

    #[test]
    fn a_missing_row_is_absent_rather_than_zero() {
        assert_eq!(lookup("header", "nothing_here"), None);
    }
}
