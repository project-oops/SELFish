//! Packages: the outer container, its entry table, and the builder that writes one.
//!
//! ```text
//! .pkg  ->  header + entry table
//!       ->  filesystem image at the offset the header names (encrypted)
//!       ->  the filesystem: files, each executable a signed container
//! ```
//!
//! The header and entry table are big-endian, unlike the executable container. Key material
//! comes from the public fake-package keyset only; retail packages are out of scope.

#![forbid(unsafe_code)]

pub mod derive;
pub mod keys;
pub mod keystone;
pub mod licence;
pub mod playgo;
pub mod sfo;
pub mod wrap;
pub mod write;

use core::fmt;

/// The four bytes a package begins with: `\x7fCNT`.
pub const MAGIC: [u8; 4] = [0x7F, 0x43, 0x4E, 0x54];

/// The other package magic, `FIH`.
///
/// A Prospero-generation package format distinct from the one above, not parsed here. It is
/// recognised so that it produces [`PackageError::UnsupportedFormat`] rather than "not a
/// package": the hardware installs it, and reading it with this layout yields plausible garbage.
pub const MAGIC_ALTERNATE: [u8; 4] = [0x7F, 0x46, 0x49, 0x48];

/// Offset of the entry count.
pub const ENTRY_COUNT_OFFSET: usize = 0x10;

/// Offset of the entry table's own offset.
pub const TABLE_OFFSET_OFFSET: usize = 0x18;

/// Size of one entry.
pub const ENTRY_SIZE: usize = 0x20;

/// Offset of the field holding where the filesystem image begins.
///
/// A big-endian `u64`. The same value appears as 32-bit mirrors at `0x7C` and `0x414` (the low
/// half of this one); the widest is read. The offset varies per package and is not the
/// `0x700000` an Orbis-generation extractor hardcodes.
pub const IMAGE_OFFSET_FIELD: usize = 0x410;

/// Offset of the content id in the header.
pub const CONTENT_ID_OFFSET: usize = 0x40;

/// The 32-bit mirrors of [`IMAGE_OFFSET_FIELD`].
pub const IMAGE_OFFSET_MIRRORS: [usize; 2] = [0x7C, 0x414];

/// One entry in a package's table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// What this entry is. See [`entry_id`].
    pub id: u32,
    /// Offset of this entry's name in the name table.
    pub name_offset: u32,
    /// First flags word. Bit 31 marks the entry encrypted - see [`keys::FLAG_ENCRYPTED`].
    pub flags1: u32,
    /// Second flags word. Bits 12-15 name the key - see [`keys::key_index`].
    pub flags2: u32,
    /// Where its data begins, from the start of the file.
    pub offset: u32,
    /// How many bytes.
    pub size: u32,
}

impl Entry {
    /// The six big-endian words of a table row, in field order.
    const FIELDS: [usize; 6] = [0x00, 0x04, 0x08, 0x0C, 0x10, 0x14];

    /// Read one from a table row, or `None` if the row is short.
    fn from_row(row: &[u8]) -> Option<Self> {
        let [id, name_offset, flags1, flags2, offset, size] =
            Self::FIELDS.map(|at| selfish_bytes::read_be(row, at));
        Some(Self {
            id: id?,
            name_offset: name_offset?,
            flags1: flags1?,
            flags2: flags2?,
            offset: offset?,
            size: size?,
        })
    }

    /// The table row a reader finds for this entry. The trailing eight bytes are zero.
    pub(crate) fn row(&self) -> [u8; ENTRY_SIZE] {
        let mut row = [0_u8; ENTRY_SIZE];
        let values = [
            self.id,
            self.name_offset,
            self.flags1,
            self.flags2,
            self.offset,
            self.size,
        ];
        for (at, value) in Self::FIELDS.into_iter().zip(values) {
            // A row holds every field.
            let _ = selfish_bytes::write_be(&mut row, at, value);
        }
        row
    }

    /// Whether this entry's data is encrypted.
    #[must_use]
    pub const fn is_encrypted(&self) -> bool {
        self.flags1 & keys::FLAG_ENCRYPTED != 0
    }

    /// Which key it declares.
    #[must_use]
    pub const fn key_index(&self) -> u32 {
        keys::key_index(self.flags2)
    }
}

/// Entry identifiers seen in every package examined.
///
/// Only ids with a cited or observed meaning are named; the rest appear only in
/// [`ALWAYS_PRESENT`](entry_id::ALWAYS_PRESENT).
pub mod entry_id {
    /// Entry keys. Part of recovering the filesystem key.
    pub const ENTRY_KEYS: u32 = 0x10;
    /// Image key. The other part.
    pub const IMAGE_KEY: u32 = 0x20;
    /// The entry that is eight kilobytes of zero in every package examined.
    pub const PARAM_SFO_ZEROS: u32 = 0x409;
    /// The licence, a RIF. Computed by `licence::Licence::build`.
    pub const LICENSE_DAT: u32 = 0x400;
    /// The shorter licence record.
    pub const LICENSE_INFO: u32 = 0x401;
    /// `PARAM.SFO` - the title metadata table.
    ///
    /// The entry begins with the PSF magic `00 50 53 46`. It is a package entry, not a file
    /// inside the filesystem. (`data/pkg-format.tsv`, `entry_content` rows)
    pub const PARAM_SFO: u32 = 0x1000;

    /// Identifiers present in every package examined: the minimum a package carries.
    pub const ALWAYS_PRESENT: [u32; 14] = [
        0x1, 0x10, 0x20, 0x80, 0x100, 0x200, 0x400, 0x401, 0x409, 0x1000, 0x1001, 0x1002, 0x1003,
        0x1200,
    ];
}

/// A parsed package, borrowing its bytes.
#[derive(Debug)]
pub struct Package<'a> {
    bytes: &'a [u8],
    entries: Vec<Entry>,
    table_at: usize,
}

impl<'a> Package<'a> {
    /// Parse the outer container.
    ///
    /// # Errors
    ///
    /// If the magic is wrong, or the entry table runs past the end of what was supplied.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, PackageError> {
        let magic = bytes.get(..4).ok_or(PackageError::TooShort)?;
        if magic == MAGIC_ALTERNATE {
            return Err(PackageError::UnsupportedFormat);
        }
        if magic != MAGIC {
            let mut found = [0_u8; 4];
            found.copy_from_slice(magic);
            return Err(PackageError::NotAPackage(found));
        }

        let word = |at| selfish_bytes::read_be::<u32>(bytes, at).ok_or(PackageError::TooShort);
        let count = word(ENTRY_COUNT_OFFSET)?;
        let table = word(TABLE_OFFSET_OFFSET)?;

        // The count is bounded against the supplied bytes before it sizes an allocation.
        let table_at = usize::try_from(table).map_err(|_| PackageError::TableOutOfBounds)?;
        let needed = usize::try_from(count)
            .ok()
            .and_then(|n| n.checked_mul(ENTRY_SIZE))
            .and_then(|n| n.checked_add(table_at))
            .ok_or(PackageError::TableOutOfBounds)?;
        if needed > bytes.len() {
            return Err(PackageError::TableOutOfBounds);
        }

        let mut entries = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
        for index in 0..count {
            let at = usize::try_from(index)
                .ok()
                .and_then(|i| i.checked_mul(ENTRY_SIZE))
                .and_then(|o| o.checked_add(table_at))
                .ok_or(PackageError::TableOutOfBounds)?;
            let row = bytes.get(at..).ok_or(PackageError::TooShort)?;
            entries.push(Entry::from_row(row).ok_or(PackageError::TooShort)?);
        }

        Ok(Self {
            bytes,
            entries,
            table_at,
        })
    }

    /// The entry table.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// One entry by identifier.
    #[must_use]
    pub fn entry(&self, id: u32) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The table row describing an entry, as thirty-two raw bytes.
    ///
    /// Distinct from [`entry_bytes`](Self::entry_bytes), the data the row points at. The key
    /// derivation hashes the row; hashing the data yields a plausible wrong key.
    #[must_use]
    pub fn entry_row(&self, entry: &Entry) -> Option<&'a [u8]> {
        let index = self.entries.iter().position(|e| e.id == entry.id)?;
        let at = index.checked_mul(ENTRY_SIZE)?.checked_add(self.table_at)?;
        self.bytes.get(at..at.checked_add(ENTRY_SIZE)?)
    }
    /// The bytes of an entry, if they are within what was supplied.
    #[must_use]
    pub fn entry_bytes(&self, entry: &Entry) -> Option<&'a [u8]> {
        let at = usize::try_from(entry.offset).ok()?;
        let len = usize::try_from(entry.size).ok()?;
        self.bytes.get(at..at.checked_add(len)?)
    }

    /// The whole package, for anything measured against the file rather than an entry.
    ///
    /// The filesystem image is described by an offset and runs to the end of the file, so a
    /// caller digesting it needs the bytes rather than an entry.
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// The content id, from the header.
    ///
    /// Thirty-six bytes at `0x40`, untrimmed: it is an input to the key derivation, and any
    /// NUL padding is part of what gets hashed.
    #[must_use]
    pub fn content_id(&self) -> &'a [u8] {
        self.bytes
            .get(CONTENT_ID_OFFSET..CONTENT_ID_OFFSET.saturating_add(keys::CONTENT_ID_LEN))
            .unwrap_or_default()
    }

    /// Where the filesystem image begins, from the header.
    ///
    /// # Errors
    ///
    /// If the header is shorter than the field.
    pub fn image_offset(&self) -> Result<u64, PackageError> {
        let end = IMAGE_OFFSET_FIELD
            .checked_add(8)
            .ok_or(PackageError::TooShort)?;
        let raw = self
            .bytes
            .get(IMAGE_OFFSET_FIELD..end)
            .ok_or(PackageError::TooShort)?;
        let mut out = [0_u8; 8];
        out.copy_from_slice(raw);
        Ok(u64::from_be_bytes(out))
    }

    /// Which of the always-present identifiers this package is missing.
    ///
    /// Empty for every package examined. A non-empty answer is reported, not treated as an
    /// error.
    #[must_use]
    pub fn missing_expected_entries(&self) -> Vec<u32> {
        entry_id::ALWAYS_PRESENT
            .iter()
            .copied()
            .filter(|id| self.entry(*id).is_none())
            .collect()
    }
}

/// Why a package could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageError {
    /// Shorter than the field being read.
    TooShort,
    /// The first four bytes are not a package's.
    NotAPackage([u8; 4]),
    /// The entry does not declare itself encrypted, so there is nothing to decrypt.
    NotEncrypted(u32),
    /// The entry declares a key this crate cannot locate.
    ///
    /// Refused rather than decrypted with a guessed key, since wrong output is not detectable.
    UnknownKeyIndex(u32, u32),
    /// A package, but in the other format - see [`MAGIC_ALTERNATE`].
    ///
    /// Separate from [`Self::NotAPackage`]: it is a real package this crate does not read.
    UnsupportedFormat,
    /// The entry table runs past the end of the supplied bytes.
    TableOutOfBounds,
    /// The committed keyset could not be read.
    KeysUnreadable,
    /// A key or IV is not the length the cipher requires.
    BadKey,
    /// A key entry this package should carry is absent.
    MissingEntry(u32),
    /// A key entry is present but shorter than the derivation needs.
    EntryTruncated(u32),
    /// The key derivation produced a malformed block.
    ///
    /// Almost always means the package is retail: its image key is encrypted under a key only
    /// the vendor holds.
    NotAFakePackage,
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "shorter than the field being read"),
            Self::BadKey => write!(f, "a key or IV is not the length the cipher requires"),
            Self::NotEncrypted(id) => write!(f, "entry {id:#x} is not encrypted"),
            Self::UnknownKeyIndex(id, index) => write!(
                f,
                "entry {id:#x} declares key index {index}, which this crate cannot locate"
            ),
            Self::UnsupportedFormat => write!(
                f,
                "a package in the other format ({MAGIC_ALTERNATE:02X?}), which this crate does not read"
            ),
            Self::NotAPackage(found) => write!(
                f,
                "not a package: begins {:02x} {:02x} {:02x} {:02x}",
                found.first().copied().unwrap_or(0),
                found.get(1).copied().unwrap_or(0),
                found.get(2).copied().unwrap_or(0),
                found.get(3).copied().unwrap_or(0)
            ),
            Self::KeysUnreadable => write!(f, "the committed keyset could not be read"),
            Self::MissingEntry(id) => {
                write!(f, "no entry {id:#x}, which the key derivation needs")
            }
            Self::EntryTruncated(id) => {
                write!(f, "entry {id:#x} is shorter than the key derivation needs")
            }
            Self::NotAFakePackage => write!(
                f,
                "the key derivation produced a malformed block; this is almost certainly a \
                 retail package, which cannot be opened with the public keyset"
            ),
            Self::TableOutOfBounds => {
                write!(f, "the entry table runs past the end of the file")
            }
        }
    }
}

impl std::error::Error for PackageError {}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    reason = "fixture builders read better indexed, and a panic here is the test failing"
)]
mod tests {
    use super::{Entry, MAGIC, MAGIC_ALTERNATE, Package, PackageError, entry_id};

    /// A package with the given entry identifiers.
    fn sample(ids: &[u32]) -> Vec<u8> {
        let table_at = 0x2A80_usize;
        let mut out = vec![0_u8; table_at + ids.len() * super::ENTRY_SIZE + 0x100];
        out[..4].copy_from_slice(&MAGIC);
        out[0x10..0x14].copy_from_slice(&(ids.len() as u32).to_be_bytes());
        out[0x18..0x1C].copy_from_slice(&(table_at as u32).to_be_bytes());
        for (index, id) in ids.iter().enumerate() {
            let at = table_at + index * super::ENTRY_SIZE;
            out[at..at + 4].copy_from_slice(&id.to_be_bytes());
            // Point every entry at a byte inside the file so `entry_bytes` is exercisable.
            out[at + 0x10..at + 0x14].copy_from_slice(&(table_at as u32).to_be_bytes());
            out[at + 0x14..at + 0x18].copy_from_slice(&4_u32.to_be_bytes());
        }
        out
    }

    /// The header's counts and offsets are read big-endian.
    #[test]
    fn the_header_is_read_big_endian() {
        let bytes = sample(&[0x1, 0x10, 0x20]);
        let package = Package::parse(&bytes).expect("parses");
        assert_eq!(package.entries().len(), 3);
    }

    /// A table row parses into its id, offset and size, and its data is reachable.
    #[test]
    fn entries_carry_their_identifier_offset_and_size() {
        let bytes = sample(&[entry_id::IMAGE_KEY]);
        let package = Package::parse(&bytes).expect("parses");
        let entry = package.entry(entry_id::IMAGE_KEY).expect("present");
        assert_eq!(
            *entry,
            Entry {
                id: entry_id::IMAGE_KEY,
                name_offset: 0,
                flags1: 0,
                flags2: 0,
                offset: 0x2A80,
                size: 4
            }
        );
        assert_eq!(package.entry_bytes(entry).map(<[u8]>::len), Some(4));
    }

    /// A wrong magic is reported with the bytes found.
    #[test]
    fn an_executable_is_reported_as_such_rather_than_as_a_bad_package() {
        let mut bytes = vec![0_u8; 64];
        bytes[..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
        assert_eq!(
            Package::parse(&bytes).expect_err("not a package"),
            PackageError::NotAPackage([0x7F, b'E', b'L', b'F'])
        );
    }

    /// An entry count larger than the file is refused before it sizes an allocation.
    #[test]
    fn a_count_larger_than_the_file_is_refused_before_it_is_allocated() {
        let mut bytes = sample(&[0x1]);
        bytes[0x10..0x14].copy_from_slice(&0x00FF_FFFF_u32.to_be_bytes());
        assert_eq!(
            Package::parse(&bytes).expect_err("refused"),
            PackageError::TableOutOfBounds
        );
    }

    /// A table offset past the end of the file is refused.
    #[test]
    fn a_table_offset_past_the_end_is_refused() {
        let mut bytes = sample(&[0x1]);
        bytes[0x18..0x1C].copy_from_slice(&0x00FF_FFFF_u32.to_be_bytes());
        assert_eq!(
            Package::parse(&bytes).expect_err("refused"),
            PackageError::TableOutOfBounds
        );
    }

    /// A missing always-present entry is reported by id, not as an error.
    #[test]
    fn a_package_missing_an_expected_entry_reports_which() {
        let bytes = sample(&[0x1, 0x10]);
        let package = Package::parse(&bytes).expect("parses");
        let missing = package.missing_expected_entries();
        assert!(missing.contains(&entry_id::IMAGE_KEY));
        assert!(!missing.contains(&entry_id::ENTRY_KEYS));
    }

    /// A package with every always-present entry reports none missing.
    #[test]
    fn a_package_with_every_expected_entry_reports_none_missing() {
        let bytes = sample(&entry_id::ALWAYS_PRESENT);
        let package = Package::parse(&bytes).expect("parses");
        assert!(package.missing_expected_entries().is_empty());
    }

    /// Input shorter than the magic is refused, not read past.
    #[test]
    fn truncation_is_refused_rather_than_read_past() {
        assert_eq!(
            Package::parse(&[0x7F, 0x43]).expect_err("too short"),
            PackageError::TooShort
        );
    }

    /// The alternate package magic is reported as unsupported, not as not-a-package.
    #[test]
    fn the_other_package_format_is_named_rather_than_called_not_a_package() {
        let mut bytes = vec![0_u8; 0x100];
        bytes[..4].copy_from_slice(&MAGIC_ALTERNATE);
        assert_eq!(
            Package::parse(&bytes).unwrap_err(),
            PackageError::UnsupportedFormat
        );
    }

    /// The two package magics are distinct and the alternate one spells `FIH`.
    #[test]
    fn the_two_magics_are_distinct_and_neither_is_the_other() {
        assert_ne!(MAGIC, MAGIC_ALTERNATE);
        assert_eq!(MAGIC_ALTERNATE, [0x7F, b'F', b'I', b'H']);
    }
}
