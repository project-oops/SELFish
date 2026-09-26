//! `PARAM.SFO` - a key-value table, read and written.
//!
//! Five fields of header, then three tables: an index, the keys, and the values. Every
//! offset in the index is relative to one of the two table starts, not to the file.
//!
//! `data/sfo-format.tsv` describes the container only; which keys a title carries is
//! convention and belongs to the consumer.
//!
//! A parse and a write round-trip byte for byte. The key table is padded to a multiple of
//! four and the file is not padded; see [`Sfo::to_bytes`].

use core::fmt;

use crate::table;

/// The header, in bytes.
const HEADER_SIZE: usize = 20;
/// One index entry, in bytes.
const INDEX_SIZE: usize = 16;
/// The key table is padded so the data table starts on a multiple of this.
///
/// Read from the table, whose row records the measurement that settled it.
fn key_table_alignment() -> usize {
    usize::from(table::u16_at("layout", "key_table_alignment"))
}

/// What a value is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// UTF-8 with no terminator.
    Utf8Special,
    /// UTF-8, null-terminated. Almost everything.
    Utf8,
    /// A little-endian `u32`.
    Integer,
    /// Something else. Kept rather than refused - see [`Value::Unknown`].
    Other(u16),
}

impl Format {
    /// Read a format code.
    #[must_use]
    pub fn from_code(code: u16) -> Self {
        if code == table::u16_at("format", "utf8_special") {
            Self::Utf8Special
        } else if code == table::u16_at("format", "utf8") {
            Self::Utf8
        } else if code == table::u16_at("format", "integer") {
            Self::Integer
        } else {
            Self::Other(code)
        }
    }

    /// The code this format is written as.
    #[must_use]
    pub fn code(self) -> u16 {
        match self {
            Self::Utf8Special => table::u16_at("format", "utf8_special"),
            Self::Utf8 => table::u16_at("format", "utf8"),
            Self::Integer => table::u16_at("format", "integer"),
            Self::Other(code) => code,
        }
    }
}

/// One value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Text, null-terminated. Almost everything.
    ///
    /// The terminator is not part of this string; it is added on the way out.
    Text(String),
    /// Text with no terminator.
    ///
    /// A separate variant rather than a flag, so a value cannot be held alongside a format it
    /// disagrees with; the variant determines the format code written.
    TextUnterminated(String),
    /// Bytes in the unterminated format, which is not always text.
    ///
    /// `utf8_special` is a length and no terminator. Older-generation saves put text in it;
    /// Prospero-generation saves also put `ACCOUNT_ID`, eight bytes of user id, in it. This and [`Self::TextUnterminated`] both write `utf8_special`; a parse produces
    /// this one when the bytes do not decode as UTF-8. [`Self::as_bytes`] answers for both.
    Binary(Vec<u8>),
    /// A number.
    Integer(u32),
    /// A format this crate does not interpret, kept as bytes.
    ///
    /// Kept rather than refused, so one unneeded key does not fail the file. The code is
    /// carried so a writer puts it back unchanged.
    Unknown(u16, Vec<u8>),
}

impl Value {
    /// The text, for either kind of text value.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) | Self::TextUnterminated(text) => Some(text),
            _ => None,
        }
    }

    /// The value as the file holds it, for everything except a number.
    ///
    /// The terminator is not included; [`Self::Text`] puts it back on the way out.
    ///
    /// A caller after raw bytes need not know which variant a parse chose: some `ACCOUNT_ID`
    /// values decode as UTF-8 and parse as [`Self::TextUnterminated`]. `None` for a number,
    /// where [`Self::as_integer`] is the accessor.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Text(text) | Self::TextUnterminated(text) => Some(text.as_bytes()),
            Self::Binary(bytes) | Self::Unknown(_, bytes) => Some(bytes),
            Self::Integer(_) => None,
        }
    }

    /// The number, for an integer value.
    #[must_use]
    pub const fn as_integer(&self) -> Option<u32> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }
}

/// One key and its value, with the sizes the file stated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The key.
    pub key: String,
    /// The value.
    pub value: Value,
    /// Bytes reserved for the value.
    ///
    /// Not derivable from the value: a title reserves 128 bytes for a name that uses 12, and
    /// shrinking `max_length` to fit changes the file.
    pub reserved: u32,
}

impl Entry {
    /// A null-terminated text entry, reserving exactly what it needs.
    #[must_use]
    pub fn text(key: &str, value: &str) -> Self {
        let reserved = u32::try_from(value.len().saturating_add(1)).unwrap_or(u32::MAX);
        Self {
            key: key.to_owned(),
            value: Value::Text(value.to_owned()),
            reserved,
        }
    }

    /// A text entry reserving a fixed number of bytes.
    #[must_use]
    pub fn text_reserving(key: &str, value: &str, reserved: u32) -> Self {
        Self {
            key: key.to_owned(),
            value: Value::Text(value.to_owned()),
            reserved,
        }
    }

    /// An integer entry.
    #[must_use]
    pub fn integer(key: &str, value: u32) -> Self {
        Self {
            key: key.to_owned(),
            value: Value::Integer(value),
            reserved: 4,
        }
    }

    /// The format code this entry is written as.
    ///
    /// Derived from the value, which distinguishes terminated from unterminated text.
    #[must_use]
    pub fn format(&self) -> Format {
        match &self.value {
            Value::Text(_) => Format::Utf8,
            Value::TextUnterminated(_) | Value::Binary(_) => Format::Utf8Special,
            Value::Integer(_) => Format::Integer,
            Value::Unknown(code, _) => Format::Other(*code),
        }
    }

    /// The bytes this entry's value occupies, padded to its reserved length.
    fn bytes(&self) -> Vec<u8> {
        let mut out = match &self.value {
            Value::Text(text) => {
                let mut bytes = text.as_bytes().to_vec();
                bytes.push(0);
                bytes
            }
            Value::TextUnterminated(text) => text.as_bytes().to_vec(),

            Value::Integer(value) => value.to_le_bytes().to_vec(),
            Value::Binary(bytes) | Value::Unknown(_, bytes) => bytes.clone(),
        };
        out.resize(
            out.len().max(usize::try_from(self.reserved).unwrap_or(0)),
            0,
        );
        out
    }

    /// The used length, which is what `length` in the index records.
    fn used(&self) -> u32 {
        match &self.value {
            Value::Text(text) => u32::try_from(text.len().saturating_add(1)).unwrap_or(u32::MAX),
            Value::TextUnterminated(text) => u32::try_from(text.len()).unwrap_or(u32::MAX),

            Value::Integer(_) => 4,
            Value::Binary(bytes) | Value::Unknown(_, bytes) => {
                u32::try_from(bytes.len()).unwrap_or(u32::MAX)
            }
        }
    }
}

/// A parsed `PARAM.SFO`.
///
/// Ordered, not a map: entry order is part of the file, and sorting would break the
/// byte-for-byte round trip.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sfo {
    entries: Vec<Entry>,
}

impl Sfo {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every entry, in file order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Add an entry, replacing any existing one with the same key in place.
    ///
    /// In place rather than appended, so editing a value does not reorder the file.
    pub fn set(&mut self, entry: Entry) {
        match self.entries.iter_mut().find(|held| held.key == entry.key) {
            Some(held) => *held = entry,
            None => self.entries.push(entry),
        }
    }

    /// Look a key up.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| &entry.value)
    }

    /// Look a key up as text.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_text)
    }

    /// Look a key up as the bytes the file holds, whatever kind of value it is.
    ///
    /// For `ACCOUNT_ID` this is its eight bytes exactly: not endian-swapped, not rendered as a
    /// number, not shortened by a trailing zero. Interpreting them is the consumer's choice.
    ///
    /// `None` for an integer parameter, where [`Value::as_integer`] is the accessor. (D090)
    #[must_use]
    pub fn bytes(&self, key: &str) -> Option<&[u8]> {
        self.get(key).and_then(Value::as_bytes)
    }

    /// Read one.
    ///
    /// # Errors
    ///
    /// If the magic is wrong, or any table runs past the end of the file.
    pub fn parse(bytes: &[u8]) -> Result<Self, SfoError> {
        let magic = table::bytes_at("header", "magic");
        if bytes.get(..magic.len()) != Some(magic.as_slice()) {
            return Err(SfoError::NotAnSfo);
        }
        let keys_at = usize::try_from(le::<u32>(bytes, 8)?).map_err(|_| SfoError::OutOfRange)?;
        let data_at = usize::try_from(le::<u32>(bytes, 12)?).map_err(|_| SfoError::OutOfRange)?;
        let count = usize::try_from(le::<u32>(bytes, 16)?).map_err(|_| SfoError::OutOfRange)?;

        let mut entries = Vec::with_capacity(count.min(1024));
        for index in 0..count {
            let at = HEADER_SIZE
                .checked_add(index.checked_mul(INDEX_SIZE).ok_or(SfoError::OutOfRange)?)
                .ok_or(SfoError::OutOfRange)?;
            let key_offset = usize::from(le::<u16>(bytes, at)?);
            let format = Format::from_code(le::<u16>(bytes, at.saturating_add(2))?);
            let length = le::<u32>(bytes, at.saturating_add(4))?;
            let reserved = le::<u32>(bytes, at.saturating_add(8))?;
            let data_offset =
                usize::try_from(le::<u32>(bytes, at.saturating_add(12))?).unwrap_or(usize::MAX);

            let key = string_at(bytes, keys_at.saturating_add(key_offset))?;
            let start = data_at.saturating_add(data_offset);
            let used = usize::try_from(length).unwrap_or(0);
            let raw = bytes
                .get(start..start.saturating_add(used))
                .ok_or(SfoError::OutOfRange)?;

            entries.push(Entry {
                key,
                value: value_of(format, raw)?,
                reserved,
            });
        }
        Ok(Self { entries })
    }

    /// Write one.
    ///
    /// Reproduces the layout both cited sources implement, including where the padding goes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let count = self.entries.len();
        let index_end = HEADER_SIZE.saturating_add(count.saturating_mul(INDEX_SIZE));

        let mut keys = Vec::new();
        let mut key_offsets = Vec::with_capacity(count);
        for entry in &self.entries {
            key_offsets.push(u16::try_from(keys.len()).unwrap_or(u16::MAX));
            keys.extend_from_slice(entry.key.as_bytes());
            keys.push(0);
        }

        let mut data = Vec::new();
        let mut data_offsets = Vec::with_capacity(count);
        for entry in &self.entries {
            data_offsets.push(u32::try_from(data.len()).unwrap_or(u32::MAX));
            data.extend_from_slice(&entry.bytes());
        }

        // The key table is padded to a multiple of four and the file is not padded, as every
        // measured file shows; the cited C writer's pad-the-file-to-sixteen rule does not
        // match them (`data/sfo-format.tsv`, `key_table_alignment`).
        keys.resize(keys.len().next_multiple_of(key_table_alignment()), 0);
        let size = index_end
            .saturating_add(keys.len())
            .saturating_add(data.len());

        let mut out = Vec::with_capacity(size);
        out.extend_from_slice(&table::bytes_at("header", "magic"));
        out.extend_from_slice(&table::bytes_at("header", "version"));
        out.extend_from_slice(&u32::try_from(index_end).unwrap_or(u32::MAX).to_le_bytes());
        out.extend_from_slice(
            &u32::try_from(index_end.saturating_add(keys.len()))
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        out.extend_from_slice(&u32::try_from(count).unwrap_or(u32::MAX).to_le_bytes());

        for (index, entry) in self.entries.iter().enumerate() {
            out.extend_from_slice(
                &key_offsets
                    .get(index)
                    .copied()
                    .unwrap_or_default()
                    .to_le_bytes(),
            );
            out.extend_from_slice(&entry.format().code().to_le_bytes());
            out.extend_from_slice(&entry.used().to_le_bytes());
            out.extend_from_slice(&entry.reserved.max(entry.used()).to_le_bytes());
            out.extend_from_slice(
                &data_offsets
                    .get(index)
                    .copied()
                    .unwrap_or_default()
                    .to_le_bytes(),
            );
        }
        out.extend_from_slice(&keys);
        out.extend_from_slice(&data);
        out
    }
}

fn value_of(format: Format, raw: &[u8]) -> Result<Value, SfoError> {
    Ok(match format {
        Format::Utf8 => {
            // The stated length includes the terminator, so trailing zeroes are not the value.
            // All are trimmed, which also covers a writer that reserved more than it used.
            let end = raw
                .iter()
                .rposition(|byte| *byte != 0)
                .map_or(0, |at| at.saturating_add(1));
            let text = core::str::from_utf8(raw.get(..end).unwrap_or_default())
                .map_err(|_| SfoError::NotUtf8)?
                .to_owned();
            Value::Text(text)
        }
        // The unterminated format's length is exact, so trailing zeroes are content and
        // nothing is trimmed. Bytes that are not UTF-8 (an `ACCOUNT_ID`) are kept as binary.
        Format::Utf8Special => core::str::from_utf8(raw).map_or_else(
            |_| Value::Binary(raw.to_vec()),
            |text| Value::TextUnterminated(text.to_owned()),
        ),
        Format::Integer => Value::Integer(le(raw, 0)?),
        Format::Other(code) => Value::Unknown(code, raw.to_vec()),
    })
}

fn string_at(bytes: &[u8], at: usize) -> Result<String, SfoError> {
    let rest = bytes.get(at..).ok_or(SfoError::OutOfRange)?;
    let end = rest
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(rest.len());
    let text = rest.get(..end).ok_or(SfoError::OutOfRange)?;
    Ok(core::str::from_utf8(text)
        .map_err(|_| SfoError::NotUtf8)?
        .to_owned())
}

/// A little-endian field, with a short read as this crate's error.
fn le<T: selfish_bytes::Int>(bytes: &[u8], at: usize) -> Result<T, SfoError> {
    selfish_bytes::read_le(bytes, at).ok_or(SfoError::OutOfRange)
}

/// What can go wrong reading one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SfoError {
    /// The magic is not `\0PSF`.
    NotAnSfo,
    /// A table or value runs past the end of the file.
    OutOfRange,
    /// A key or a text value is not valid UTF-8.
    NotUtf8,
}

impl fmt::Display for SfoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnSfo => write!(f, "not a PARAM.SFO"),
            Self::OutOfRange => write!(f, "a table runs past the end of the file"),
            Self::NotUtf8 => write!(f, "a key or value is not UTF-8"),
        }
    }
}

impl std::error::Error for SfoError {}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{Entry, Format, Sfo, SfoError, Value};

    /// A written table parses back to what was written.
    #[test]
    fn a_written_table_parses_back_to_what_was_written() {
        let mut sfo = Sfo::new();
        sfo.set(Entry::text("TITLE_ID", "PPSA01650"));
        sfo.set(Entry::text("TITLE", "A Name"));
        sfo.set(Entry::integer("PARENTAL_LEVEL", 1));

        let parsed = Sfo::parse(&sfo.to_bytes()).expect("a table");
        assert_eq!(parsed, sfo, "a round trip is the test");
    }

    /// The key table pads to a multiple of four and the file ends at the last value.
    #[test]
    fn the_key_table_pads_to_four_and_the_file_does_not_pad_at_all() {
        // `CATEGORY` plus its terminator is nine bytes, so the key table is padded to twelve.
        let mut sfo = Sfo::new();
        sfo.set(Entry::text("CATEGORY", "gd"));
        let bytes = sfo.to_bytes();

        let keys_at = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let data_at = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        assert_eq!(keys_at, 20 + 16, "header then one index entry");
        assert_eq!(data_at - keys_at, 12, "nine bytes of key, padded to twelve");
        assert_eq!(
            bytes.len(),
            data_at + 3,
            "and the file itself is not padded"
        );
    }

    /// The padding rule reproduces every measured key-table length.
    #[test]
    fn the_padding_rule_reproduces_every_measured_file() {
        // Key-table lengths measured across three hardware generations' titles and saves.
        for (strings, expected) in [
            (100_usize, 100_usize),
            (58, 60),
            (32, 32),
            (121, 124),
            (141, 144),
            (155, 156),
            (461, 464),
        ] {
            assert_eq!(
                strings.next_multiple_of(super::key_table_alignment()),
                expected,
                "a key table of {strings} bytes is written as {expected}"
            );
        }
    }

    /// Entry order is preserved and replacing a key does not move it.
    #[test]
    fn order_is_preserved_and_editing_does_not_reorder() {
        let mut sfo = Sfo::new();
        sfo.set(Entry::text("A", "1"));
        sfo.set(Entry::text("B", "2"));
        sfo.set(Entry::text("A", "3"));

        let keys: Vec<_> = sfo
            .entries()
            .iter()
            .map(|entry| entry.key.as_str())
            .collect();
        assert_eq!(keys, ["A", "B"], "replaced in place, not appended");
        assert_eq!(sfo.text("A"), Some("3"));
    }

    /// A reserved length larger than the value survives a round trip.
    #[test]
    fn a_reserved_length_larger_than_the_value_survives_a_round_trip() {
        let mut sfo = Sfo::new();
        sfo.set(Entry::text_reserving("TITLE", "Short", 128));
        let bytes = sfo.to_bytes();
        let parsed = Sfo::parse(&bytes).expect("a table");

        assert_eq!(parsed.entries()[0].reserved, 128);
        assert_eq!(parsed.text("TITLE"), Some("Short"));
        assert_eq!(parsed.to_bytes(), bytes, "and writes back identically");
    }

    /// Unterminated text keeps format `0x0004` and gains no terminator through a round trip.
    #[test]
    fn unterminated_text_stays_unterminated_through_a_round_trip() {
        let mut sfo = Sfo::new();
        sfo.set(Entry {
            key: "PARAMS".to_owned(),
            value: Value::TextUnterminated("abcd".to_owned()),
            reserved: 4,
        });
        let bytes = sfo.to_bytes();
        assert_eq!(&bytes[bytes.len() - 4..], b"abcd", "no terminator written");

        let parsed = Sfo::parse(&bytes).expect("a table");
        assert_eq!(
            parsed.entries()[0].value,
            Value::TextUnterminated("abcd".to_owned())
        );
        assert_eq!(parsed.to_bytes(), bytes);
        assert_eq!(parsed.entries()[0].format(), Format::Utf8Special);
    }

    /// An unrecognised format code keeps its bytes rather than failing the file.
    #[test]
    fn an_unrecognised_format_keeps_its_bytes_rather_than_failing_the_file() {
        let mut sfo = Sfo::new();
        sfo.set(Entry {
            key: "ODD".to_owned(),
            value: Value::Unknown(0x0804, vec![1, 2, 3, 4]),
            reserved: 4,
        });
        let parsed = Sfo::parse(&sfo.to_bytes()).expect("a table");
        assert_eq!(
            parsed.entries()[0].value,
            Value::Unknown(0x0804, vec![1, 2, 3, 4])
        );
    }

    /// A file without the `\0PSF` magic is refused.
    #[test]
    fn the_wrong_magic_is_refused() {
        assert_eq!(Sfo::parse(b"NOPE").unwrap_err(), SfoError::NotAnSfo);
        assert_eq!(Sfo::parse(&[]).unwrap_err(), SfoError::NotAnSfo);
    }

    /// A truncated file is an error, not a shorter table.
    #[test]
    fn a_truncated_file_is_an_error_rather_than_a_short_table() {
        let mut sfo = Sfo::new();
        sfo.set(Entry::text("TITLE_ID", "PPSA01650"));
        let bytes = sfo.to_bytes();
        assert_eq!(
            Sfo::parse(&bytes[..bytes.len() - 8]).unwrap_err(),
            SfoError::OutOfRange
        );
    }

    /// The format codes are the ones both cited sources write.
    #[test]
    fn the_format_codes_are_the_ones_both_sources_write() {
        assert_eq!(Format::Utf8.code(), 0x0204);
        assert_eq!(Format::Utf8Special.code(), 0x0004);
        assert_eq!(Format::Integer.code(), 0x0404);
        assert_eq!(Format::from_code(0x0204), Format::Utf8);
        assert_eq!(Format::from_code(0x9999), Format::Other(0x9999));
    }

    /// Eight bytes that are not valid UTF-8, which is what a user id usually is.
    ///
    /// `0xff` cannot begin a UTF-8 sequence, so this always takes the binary path.
    const ACCOUNT_ID: [u8; 8] = [0xff, 0x01, 0x00, 0x9a, 0x2b, 0x00, 0x7c, 0xd3];

    /// A non-UTF-8 `ACCOUNT_ID` parses, reads back as its exact bytes and round-trips.
    #[test]
    fn an_account_id_reads_back_as_its_exact_eight_bytes() {
        let mut sfo = Sfo::new();
        sfo.set(Entry::text("TITLE_ID", "CUSA00001"));
        sfo.set(Entry {
            key: "ACCOUNT_ID".to_owned(),
            value: Value::Binary(ACCOUNT_ID.to_vec()),
            reserved: 8,
        });
        let bytes = sfo.to_bytes();

        let parsed = Sfo::parse(&bytes).expect("a file with a binary parameter still parses");
        assert_eq!(parsed.bytes("ACCOUNT_ID"), Some(ACCOUNT_ID.as_slice()));
        assert_eq!(parsed.text("TITLE_ID"), Some("CUSA00001"));
        assert_eq!(parsed.entries()[1].format(), Format::Utf8Special);
        assert_eq!(parsed.to_bytes(), bytes, "and writes back identically");
    }

    /// `bytes` answers for a binary value that happened to decode as text.
    #[test]
    fn bytes_answers_for_a_value_that_decoded_as_text_too() {
        let readable = *b"abcdefgh";
        let mut sfo = Sfo::new();
        sfo.set(Entry {
            key: "ACCOUNT_ID".to_owned(),
            value: Value::Binary(readable.to_vec()),
            reserved: 8,
        });
        let parsed = Sfo::parse(&sfo.to_bytes()).expect("a table");

        assert_eq!(
            parsed.entries()[0].value,
            Value::TextUnterminated("abcdefgh".to_owned()),
            "it decoded, so it came back as text"
        );
        assert_eq!(
            parsed.bytes("ACCOUNT_ID"),
            Some(readable.as_slice()),
            "and the bytes are reachable anyway"
        );
    }

    /// A trailing zero in the unterminated format is part of the value.
    #[test]
    fn a_trailing_zero_in_the_unterminated_format_is_content() {
        let ending_in_zero = [0xd3, 0x2b, 0x9a, 0x01, 0x7c, 0x00, 0xff, 0x00];
        let mut sfo = Sfo::new();
        sfo.set(Entry {
            key: "ACCOUNT_ID".to_owned(),
            value: Value::Binary(ending_in_zero.to_vec()),
            reserved: 8,
        });
        let bytes = sfo.to_bytes();
        let parsed = Sfo::parse(&bytes).expect("a table");

        assert_eq!(parsed.bytes("ACCOUNT_ID"), Some(ending_in_zero.as_slice()));
        assert_eq!(parsed.to_bytes(), bytes);
    }

    /// A `utf8` value's terminator and reserved padding are not part of the value.
    #[test]
    fn a_terminated_text_value_still_loses_its_terminator() {
        let mut sfo = Sfo::new();
        sfo.set(Entry::text_reserving("TITLE", "Name", 32));
        let parsed = Sfo::parse(&sfo.to_bytes()).expect("a table");

        assert_eq!(parsed.text("TITLE"), Some("Name"));
        assert_eq!(parsed.bytes("TITLE"), Some(b"Name".as_slice()));
    }

    /// An integer is read through `as_integer` and has no raw bytes.
    #[test]
    fn an_integer_has_no_bytes_because_its_endianness_is_already_decided() {
        let mut sfo = Sfo::new();
        sfo.set(Entry::integer("ATTRIBUTE", 0x1234));
        let parsed = Sfo::parse(&sfo.to_bytes()).expect("a table");

        assert_eq!(
            parsed.get("ATTRIBUTE").and_then(Value::as_integer),
            Some(0x1234)
        );
        assert_eq!(parsed.bytes("ATTRIBUTE"), None);
    }
}
