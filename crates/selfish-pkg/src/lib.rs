//! Packages: the outer container and its entry table.
//!
//! A package is not an archive of files. It is four nested formats:
//!
//! ```text
//! .pkg  ->  header + entry table              <- this crate, so far
//!       ->  filesystem image at 0x700000      <- encrypted
//!       ->  a compressed image inside that
//!       ->  the real filesystem: files, each executable a container
//! ```
//!
//! Only the first layer is implemented. It needs no cryptography, which is why it is
//! separable and why it comes first - the entry table can be read, checked against real
//! packages and relied on before a single cipher is involved.
//!
//! # Big-endian, unlike everything else here
//!
//! The executable container is little-endian throughout. This one is not: magic, counts and
//! offsets are all big-endian. A reader that carries one convention across both formats gets
//! an entry count in the tens of millions and no indication why.
//!
//! # What is here and what is not
//!
//! The layers below this need RSA, SHA-256, AES-CBC, AES-XTS and zlib, and **the public
//! fake-package keyset only** - retail packages use a key nobody outside the vendor has and
//! are out of scope in the same way retail signing is.

#![forbid(unsafe_code)]

pub mod derive;
pub mod keys;
pub mod keystone;
pub mod licence;
pub mod sfo;
pub mod wrap;
pub mod write;

use core::fmt;

/// The four bytes a package begins with: `\x7fCNT`.
pub const MAGIC: [u8; 4] = [0x7F, 0x43, 0x4E, 0x54];

/// The other package magic, `FIH`.
///
/// A current-generation package format, distinct from the one above and **not parsed here**.
/// Recognised only so that being handed one produces [`PackageError::UnsupportedFormat`]
/// instead of "not a package".
///
/// The distinction is not cosmetic. The console's installer accepts both, so a tool that
/// reports one of them as not-a-package is telling the user something false about a file that
/// works. And the alternative failure is worse: assuming the layout above and reading a
/// big-endian entry count out of a header that does not have one there gives a count, a
/// table, and entries - all wrong, none of it detectably so. (principle 5, D021)
pub const MAGIC_ALTERNATE: [u8; 4] = [0x7F, 0x46, 0x49, 0x48];

/// Offset of the entry count.
pub const ENTRY_COUNT_OFFSET: usize = 0x10;

/// Offset of the entry table's own offset.
pub const TABLE_OFFSET_OFFSET: usize = 0x18;

/// Size of one entry.
pub const ENTRY_SIZE: usize = 0x20;

/// Offset of the field holding where the filesystem image begins.
///
/// A big-endian `u64`. The same value appears as a 32-bit field at `0x7C` and again at
/// `0x414` - the low half of this one - in every package examined; which is authoritative
/// cannot be settled from three samples, so the widest is read and the others recorded.
///
/// # This was very nearly hardcoded, on evidence that could not have failed
///
/// A previous-generation extractor hardcodes `0x700000`, and this crate first recorded that
/// as a fixed convention "confirmed" by finding high-entropy data there in all three samples.
/// That confirmed nothing: in an encrypted package almost every offset holds high-entropy
/// data. The real values are `0x80000`, `0x580000` and `0x80000` - not fixed, not `0x700000`,
/// and named in the header all along.
pub const IMAGE_OFFSET_FIELD: usize = 0x410;

/// Offset of the content id in the header.
pub const CONTENT_ID_OFFSET: usize = 0x40;

/// The 32-bit mirrors of that field, recorded because three samples cannot rank them.
pub const IMAGE_OFFSET_MIRRORS: [usize; 2] = [0x7C, 0x414];

/// One entry in a package's table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// What this entry is. See [`entry_id`].
    pub id: u32,
    /// Offset of this entry's name in the name table.
    pub name_offset: u32,
    /// First flags word. Bit 31 marks the entry encrypted - see [`keys::FLAG_ENCRYPTED`].
    ///
    /// Read late: this crate spent a long time treating the record as three useful fields and
    /// eight bytes of padding, and the two licence entries looked like unbreakable noise the
    /// entire time. They declare themselves encrypted here. (D044)
    pub flags1: u32,
    /// Second flags word. Bits 12-15 name the key - see [`keys::key_index`].
    pub flags2: u32,
    /// Where its data begins, from the start of the file.
    pub offset: u32,
    /// How many bytes.
    pub size: u32,
}

impl Entry {
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
/// Only two have names, and they are the two an open-source extractor needs. The rest are
/// recorded as present rather than guessed at - an unnamed constant is honest, an invented
/// name is not.
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
    /// Identified rather than assumed: the entry begins `00 50 53 46`, which is the PSF
    /// magic. It is a package entry rather than a file inside the filesystem, so extracting
    /// the filesystem does not produce it. (`data/pkg-format.tsv`, `entry_content` rows)
    pub const PARAM_SFO: u32 = 0x1000;

    /// Identifiers present in all three packages examined.
    ///
    /// A minimum viable package, established by measurement rather than by specification:
    /// every one of these appeared in every sample, and nine further ids appeared in only
    /// one.
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

        let count = read_u32_be(bytes, ENTRY_COUNT_OFFSET)?;
        let table = read_u32_be(bytes, TABLE_OFFSET_OFFSET)?;

        // A count is a claim, and a wrong one asks for an allocation before anything has been
        // validated. Bounded against what is actually present rather than trusted.
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
            entries.push(Entry {
                id: read_u32_be(bytes, at)?,
                name_offset: read_u32_be(
                    bytes,
                    at.checked_add(0x04).ok_or(PackageError::TooShort)?,
                )?,
                flags1: read_u32_be(bytes, at.checked_add(0x08).ok_or(PackageError::TooShort)?)?,
                flags2: read_u32_be(bytes, at.checked_add(0x0C).ok_or(PackageError::TooShort)?)?,
                offset: read_u32_be(bytes, at.checked_add(0x10).ok_or(PackageError::TooShort)?)?,
                size: read_u32_be(bytes, at.checked_add(0x14).ok_or(PackageError::TooShort)?)?,
            });
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

    /// The **table row** describing an entry, as thirty-two raw bytes.
    ///
    /// Distinct from [`entry_bytes`](Self::entry_bytes), which is the data the row points at.
    /// Both are associated with one entry, and the key derivation hashes the *row* - worth an
    /// accessor of its own, because using the wrong one yields a key exactly as plausible and
    /// entirely wrong.
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
    /// Thirty-six bytes at `0x40`. It is an input to the key derivation, so it is read as the
    /// bytes it is rather than trimmed to a string - the NUL padding past the id is part of
    /// what gets hashed.
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
    /// Empty for every package examined. A non-empty answer is a finding rather than an
    /// error: it means either the sample set was too small or this package is unusual, and
    /// both are worth knowing before a builder is written against the list.
    #[must_use]
    pub fn missing_expected_entries(&self) -> Vec<u32> {
        entry_id::ALWAYS_PRESENT
            .iter()
            .copied()
            .filter(|id| self.entry(*id).is_none())
            .collect()
    }
}

fn read_u32_be(bytes: &[u8], at: usize) -> Result<u32, PackageError> {
    let end = at.checked_add(4).ok_or(PackageError::TooShort)?;
    let raw = bytes.get(at..end).ok_or(PackageError::TooShort)?;
    let mut out = [0_u8; 4];
    out.copy_from_slice(raw);
    Ok(u32::from_be_bytes(out))
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
    /// Refused rather than decrypted with the wrong block: output that decrypts to noise is
    /// indistinguishable from output that decrypted correctly into something unrecognised.
    UnknownKeyIndex(u32, u32),
    /// A package, but in the other format - see [`MAGIC_ALTERNATE`].
    ///
    /// Separate from [`Self::NotAPackage`] because it is a real package this crate cannot
    /// read, which is a different thing to say than "this is not a package".
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
    /// Almost always means the package is retail rather than fake, which is a wall rather
    /// than a bug: a retail image key is encrypted under a key nobody outside the vendor has.
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

    #[test]
    fn the_header_is_read_big_endian() {
        // The whole point of the note in the module header. Read little-endian, an entry
        // count of 14 becomes 234881024 and the failure is an allocation rather than a
        // parse error.
        let bytes = sample(&[0x1, 0x10, 0x20]);
        let package = Package::parse(&bytes).expect("parses");
        assert_eq!(package.entries().len(), 3);
    }

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

    #[test]
    fn an_executable_is_reported_as_such_rather_than_as_a_bad_package() {
        let mut bytes = vec![0_u8; 64];
        bytes[..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
        assert_eq!(
            Package::parse(&bytes).expect_err("not a package"),
            PackageError::NotAPackage([0x7F, b'E', b'L', b'F'])
        );
    }

    #[test]
    fn a_count_larger_than_the_file_is_refused_before_it_is_allocated() {
        // A count is a claim. Trusting one asks for an allocation sized by an attacker before
        // anything at all has been validated.
        let mut bytes = sample(&[0x1]);
        bytes[0x10..0x14].copy_from_slice(&0x00FF_FFFF_u32.to_be_bytes());
        assert_eq!(
            Package::parse(&bytes).expect_err("refused"),
            PackageError::TableOutOfBounds
        );
    }

    #[test]
    fn a_table_offset_past_the_end_is_refused() {
        let mut bytes = sample(&[0x1]);
        bytes[0x18..0x1C].copy_from_slice(&0x00FF_FFFF_u32.to_be_bytes());
        assert_eq!(
            Package::parse(&bytes).expect_err("refused"),
            PackageError::TableOutOfBounds
        );
    }

    #[test]
    fn a_package_missing_an_expected_entry_reports_which() {
        // Not an error. A package without one of these is either unusual or evidence the
        // sample set was too small, and both matter before a builder is written to the list.
        let bytes = sample(&[0x1, 0x10]);
        let package = Package::parse(&bytes).expect("parses");
        let missing = package.missing_expected_entries();
        assert!(missing.contains(&entry_id::IMAGE_KEY));
        assert!(!missing.contains(&entry_id::ENTRY_KEYS));
    }

    #[test]
    fn a_package_with_every_expected_entry_reports_none_missing() {
        let bytes = sample(&entry_id::ALWAYS_PRESENT);
        let package = Package::parse(&bytes).expect("parses");
        assert!(package.missing_expected_entries().is_empty());
    }

    #[test]
    fn truncation_is_refused_rather_than_read_past() {
        assert_eq!(
            Package::parse(&[0x7F, 0x43]).expect_err("too short"),
            PackageError::TooShort
        );
    }
    #[test]
    fn the_other_package_format_is_named_rather_than_called_not_a_package() {
        // The console's installer accepts both magics. Reporting one of them as not-a-package
        // tells the user something false about a file that works - and assuming this crate's
        // layout for it would be worse, since a big-endian read of a header that has no count
        // there yields a count, a table and entries, all wrong and none of it detectable.
        let mut bytes = vec![0_u8; 0x100];
        bytes[..4].copy_from_slice(&MAGIC_ALTERNATE);
        assert_eq!(
            Package::parse(&bytes).unwrap_err(),
            PackageError::UnsupportedFormat
        );
    }

    #[test]
    fn the_two_magics_are_distinct_and_neither_is_the_other() {
        assert_ne!(MAGIC, MAGIC_ALTERNATE);
        assert_eq!(MAGIC_ALTERNATE, [0x7F, b'F', b'I', b'H']);
    }
}
