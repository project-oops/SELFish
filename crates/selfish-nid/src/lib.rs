//! The import hash.
//!
//! A vendor module imports by a hash of the symbol name rather than by the name itself.
//!
//! ```text
//! NID = first 8 bytes of SHA-1(name || suffix), packed so `digest[0]` is the LEAST
//!       significant byte of the `u64`
//! ```
//!
//! then encoded as eleven characters of a 64-symbol alphabet with two padding bits at the
//! bottom. A full dynamic symbol name is `<encoded>#<library>#<module>`.
//!
//! The suffix, byte order, alphabet and packing are pinned by `tests/known-pairs.txt`:
//! name-and-encoding pairs taken from the resolution logs of independent open-source
//! emulators. Agreement with those pairs is why there is one implementation, shared by the
//! probes and the emulator they measure (D004).

#![forbid(unsafe_code)]

use core::fmt;

use sha1::{Digest, Sha1};

/// The committed suffix, with its provenance beside it.
const SUFFIX_TOML: &str = include_str!("../../../data/hash-suffix.toml");

/// Characters a NID is encoded with.
///
/// Standard base64 ordering with `+` and `-` as the final two. This is not the RFC 4648
/// alphabet, whose last two characters differ.
pub const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+-";

/// Characters in an encoded NID.
pub const ENCODED_LEN: usize = 11;

/// Eleven characters carry 66 bits; the low two are padding.
const PADDING_BITS: u32 = 2;

/// The hash suffix, as sixteen bytes.
///
/// Embedded at compile time: the suffix is a property of the format, not configuration.
///
/// # Panics
///
/// Never: the committed data file is embedded by `include_str!` and its shape is covered by
/// a test.
#[must_use]
pub fn suffix() -> Vec<u8> {
    parse_suffix(SUFFIX_TOML).unwrap_or_default()
}

/// Pulls `suffix_hex` out of the data file.
///
/// Parsed by hand; the file has one key and needs no TOML dependency.
fn parse_suffix(toml: &str) -> Option<Vec<u8>> {
    let line = toml
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("suffix_hex"))?;
    let hex = line.split('"').nth(1)?;
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let pair = hex.get(at..at.checked_add(2)?)?;
        out.push(u8::from_str_radix(pair, 16).ok()?);
        at = at.checked_add(2)?;
    }
    Some(out)
}

/// The hash a loader resolves a symbol by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Nid(u64);

impl Nid {
    /// Hash a symbol name with the committed suffix.
    #[must_use]
    pub fn of(name: &str) -> Self {
        Self::with_suffix(name, &suffix())
    }

    /// Hash a symbol name with a supplied suffix.
    ///
    /// Lets a test vary the suffix without the committed one being editable at run time.
    #[must_use]
    pub fn with_suffix(name: &str, suffix: &[u8]) -> Self {
        let mut hasher = Sha1::new();
        hasher.update(name.as_bytes());
        hasher.update(suffix);
        let digest = hasher.finalize();

        let mut first = [0_u8; 8];
        if let Some(head) = digest.get(..8) {
            first.copy_from_slice(head);
        }
        // `digest[0]` becomes the least significant byte of the `u64`. Stated by which end of
        // the digest lands where, because an endian word describes the packing and reads
        // differently depending on which side of it the reader stands.
        Self(u64::from_le_bytes(first))
    }

    /// The raw value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// From a raw value.
    #[must_use]
    pub const fn from_value(value: u64) -> Self {
        Self(value)
    }

    /// The eleven characters that appear in a symbol name.
    #[must_use]
    pub fn encode(self) -> String {
        let bits = u128::from(self.0) << PADDING_BITS;
        (0..ENCODED_LEN)
            .rev()
            .map(|position| {
                let shift = u32::try_from(position).unwrap_or(0).saturating_mul(6);
                let index = usize::try_from((bits >> shift) & 0x3F).unwrap_or(0);
                char::from(*ALPHABET.get(index).unwrap_or(&b'A'))
            })
            .collect()
    }

    /// Back from those eleven characters.
    ///
    /// The inverse makes the transform testable against any value, not only published pairs.
    ///
    /// # Errors
    ///
    /// If the input is not eleven characters, or carries a character outside the alphabet.
    pub fn decode(encoded: &str) -> Result<Self, DecodeError> {
        let count = encoded.chars().count();
        if count != ENCODED_LEN {
            return Err(DecodeError::WrongLength(count));
        }
        let mut bits: u128 = 0;
        for character in encoded.chars() {
            let byte =
                u8::try_from(character).map_err(|_| DecodeError::NotInAlphabet(character))?;
            let value = ALPHABET
                .iter()
                .position(|candidate| *candidate == byte)
                .ok_or(DecodeError::NotInAlphabet(character))?;
            bits = (bits << 6) | u128::try_from(value).unwrap_or(0);
        }
        Ok(Self(u64::try_from(bits >> PADDING_BITS).unwrap_or(0)))
    }
}

/// An import, as a dynamic symbol name encodes it.
///
/// Symbol names take the form `H2e8t5ScQGc#B#C`: an encoded hash, a library id and a module
/// id, all in the same alphabet. The ids index the vendor's own library and module tables,
/// not `DT_NEEDED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Import {
    /// The hash a loader resolves by.
    pub nid: Nid,
    /// Index into the module's import-library table.
    pub library_id: u16,
    /// Index into its module table.
    pub module_id: u16,
}

/// Decode a dynamic symbol name of the form `<nid>#<library>#<module>`.
///
/// `None` for anything not in that form. These modules also carry ordinary locally-defined
/// symbol names, which are not an error.
#[must_use]
pub fn decode_symbol_name(name: &str) -> Option<Import> {
    let mut parts = name.split('#');
    let (encoded, library, module) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    Some(Import {
        nid: Nid::decode(encoded).ok()?,
        library_id: decode_small(library)?,
        module_id: decode_small(module)?,
    })
}

/// Encode a library or module id the way a symbol name spells it.
///
/// The same alphabet as the hash with no padding bits: the id is a number, not a truncated
/// digest, and is one or two characters long. Zero is `A`, a real id (usually the kernel's)
/// rather than an absence.
#[must_use]
pub fn encode_index(value: u16) -> String {
    let mut value = value;
    let mut digits = Vec::new();
    loop {
        let digit = ALPHABET
            .get(usize::from(value % 64))
            .copied()
            .unwrap_or(b'A');
        digits.push(digit);
        value /= 64;
        if value == 0 {
            break;
        }
    }
    digits.reverse();
    String::from_utf8(digits).unwrap_or_default()
}

/// The full dynamic symbol name for an import: `<hash>#<library>#<module>`.
///
/// Takes a [`Nid`] rather than a name. Most imports are named and hashed; an import known only
/// by its identifier arrives as `Nid::decode(...)`, which validates it before it is spliced
/// into a symbol name.
#[must_use]
pub fn symbol_name(nid: Nid, library: u16, module: u16) -> String {
    format!(
        "{}#{}#{}",
        nid.encode(),
        encode_index(library),
        encode_index(module)
    )
}

/// The suffix, written out so a report can say which one produced its numbers.
///
/// The whole suffix rather than a hash of it: it is public and short, and tables built with
/// different suffixes are not comparable.
#[must_use]
pub fn suffix_fingerprint(suffix: &[u8]) -> String {
    use core::fmt::Write as _;
    let mut out = String::with_capacity(suffix.len().saturating_mul(2));
    for byte in suffix {
        let _ = write!(out, "{byte:02X}");
    }
    out
}

/// Read a library or module id back.
///
/// The inverse of [`encode_index`].
#[must_use]
pub fn decode_index(text: &str) -> Option<u16> {
    decode_small(text)
}

/// Decode a short id in the NID alphabet, as a plain base-64 number with no padding bits.
fn decode_small(text: &str) -> Option<u16> {
    if text.is_empty() {
        return None;
    }
    let mut value: u32 = 0;
    for byte in text.bytes() {
        let digit = ALPHABET.iter().position(|candidate| *candidate == byte)?;
        value = value
            .checked_mul(64)?
            .checked_add(u32::try_from(digit).ok()?)?;
    }
    u16::try_from(value).ok()
}

impl fmt::Display for Nid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.encode())
    }
}

/// Why an encoded NID could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Not eleven characters.
    WrongLength(usize),
    /// A character outside the 64-symbol alphabet.
    NotInAlphabet(char),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength(n) => {
                write!(f, "an encoded NID is {ENCODED_LEN} characters, this is {n}")
            }
            Self::NotInAlphabet(c) => write!(f, "`{c}` is not in the NID alphabet"),
        }
    }
}

impl std::error::Error for DecodeError {}

#[cfg(test)]
mod tests {
    use super::{
        ALPHABET, DecodeError, ENCODED_LEN, Nid, decode_index, decode_symbol_name, encode_index,
        parse_suffix, suffix, suffix_fingerprint, symbol_name,
    };

    /// The committed suffix file parses to sixteen bytes.
    #[test]
    fn the_committed_suffix_parses_to_sixteen_bytes() {
        assert_eq!(suffix().len(), 16, "the suffix is sixteen bytes");
    }

    /// A missing, odd-length or non-hex suffix is refused whole.
    #[test]
    fn a_malformed_suffix_file_is_refused_rather_than_half_read() {
        assert_eq!(parse_suffix("nothing here"), None);
        assert_eq!(
            parse_suffix("suffix_hex = \"ABC\""),
            None,
            "odd digit count"
        );
        assert_eq!(parse_suffix("suffix_hex = \"ZZ\""), None, "not hex");
    }

    /// One published pair constrains suffix, byte order, alphabet and packing at once.
    #[test]
    fn the_published_pair_holds() {
        assert_eq!(Nid::of("sceKernelLoadStartModule").encode(), "wzvqT4UqKX8");
    }

    /// Encode and decode are inverse for arbitrary values.
    #[test]
    fn encoding_round_trips_for_arbitrary_values() {
        for value in [0, 1, u64::MAX, 0x0123_4567_89AB_CDEF, 0xFEDC_BA98_7654_3210] {
            let nid = Nid::from_value(value);
            assert_eq!(
                Nid::decode(&nid.encode()),
                Ok(nid),
                "round trip for {value:#x}"
            );
        }
    }

    /// Every encoding is eleven characters drawn from the alphabet.
    #[test]
    fn every_encoding_is_eleven_characters_from_the_alphabet() {
        for value in [0, 1, u64::MAX, 0x8000_0000_0000_0000] {
            let encoded = Nid::from_value(value).encode();
            assert_eq!(encoded.chars().count(), ENCODED_LEN);
            for c in encoded.chars() {
                assert!(
                    ALPHABET.contains(&u8::try_from(c).expect("ascii")),
                    "{c} is not in the alphabet"
                );
            }
        }
    }

    /// Wrong lengths and characters outside the alphabet (including RFC 4648's `/`) are refused.
    #[test]
    fn decoding_refuses_what_it_cannot_represent() {
        assert_eq!(Nid::decode("short"), Err(DecodeError::WrongLength(5)));
        assert_eq!(
            Nid::decode("wzvqT4UqKX8x"),
            Err(DecodeError::WrongLength(12))
        );
        assert_eq!(
            Nid::decode("wzvqT4UqKX*"),
            Err(DecodeError::NotInAlphabet('*'))
        );
        assert_eq!(
            Nid::decode("wzvqT4UqKX/"),
            Err(DecodeError::NotInAlphabet('/'))
        );
    }

    /// An encoded import splits into a hash, a library index and a module index.
    #[test]
    fn an_encoded_import_splits_into_a_hash_and_two_indices() {
        let import = decode_symbol_name("wzvqT4UqKX8#B#C").expect("an import");
        assert_eq!(import.nid, Nid::of("sceKernelLoadStartModule"));
        assert_eq!(import.library_id, 1, "B is one");
        assert_eq!(import.module_id, 2, "C is two");
    }

    /// `A` decodes to index zero, a real library, not an absence.
    #[test]
    fn the_zero_index_is_a_real_index_and_not_an_absence() {
        let import = decode_symbol_name("wzvqT4UqKX8#A#A").expect("an import");
        assert_eq!(import.library_id, 0);
        assert_eq!(import.module_id, 0);
    }

    /// Ids above sixty-three take a second base-64 character.
    #[test]
    fn two_character_indices_are_read_as_base_sixty_four() {
        let import = decode_symbol_name("wzvqT4UqKX8#BA#C").expect("an import");
        assert_eq!(import.library_id, 64, "B then A is one times sixty-four");
    }

    /// A plain or malformed symbol name decodes to no import rather than an error.
    #[test]
    fn an_ordinary_symbol_name_is_not_an_import_and_not_an_error() {
        assert_eq!(decode_symbol_name("memcpy"), None);
        assert_eq!(decode_symbol_name("wzvqT4UqKX8"), None, "no ids at all");
        assert_eq!(decode_symbol_name("a#b#c#d"), None, "too many parts");
        assert_eq!(decode_symbol_name("wzvqT4UqKX8##C"), None, "an empty id");
        assert_eq!(
            decode_symbol_name("short#B#C"),
            None,
            "not eleven characters"
        );
    }

    /// The suffix is applied: an empty suffix gives a different hash.
    #[test]
    fn a_different_suffix_gives_a_different_answer() {
        let with = Nid::of("sceKernelLoadStartModule");
        let without = Nid::with_suffix("sceKernelLoadStartModule", &[]);
        assert_ne!(with, without, "the suffix must actually be applied");
    }

    /// Library and module ids round-trip through the index encoding.
    #[test]
    fn an_id_round_trips_through_the_index_encoding() {
        for value in [0_u16, 1, 63, 64, 65, 4095, u16::MAX] {
            let encoded = encode_index(value);
            assert_eq!(
                decode_index(&encoded),
                Some(value),
                "{value} encoded as {encoded:?}"
            );
        }
    }

    /// An id encodes as a short unpadded number, not through the hash encoder.
    #[test]
    fn the_index_encoding_is_not_the_hash_encoding() {
        assert_eq!(
            encode_index(0),
            "A",
            "zero is a real id, usually the kernel"
        );
        assert_eq!(encode_index(1), "B");
        assert_eq!(encode_index(64), "BA");
        assert_eq!(encode_index(63), "-");
    }

    /// A built symbol name decodes back to its hash and ids.
    #[test]
    fn a_symbol_name_round_trips_through_the_decoder() {
        let nid = Nid::of("sceKernelLoadStartModule");
        let name = symbol_name(nid, 0, 1);

        let read = decode_symbol_name(&name).expect("an import");
        assert_eq!(read.nid, nid);
        assert_eq!(read.library_id, 0);
        assert_eq!(read.module_id, 1);
    }

    /// An identifier-only import is validated by decoding before it becomes a symbol name.
    #[test]
    fn an_already_encoded_identifier_is_validated_rather_than_spliced() {
        let nid = Nid::decode("wzvqT4UqKX8").expect("a valid identifier");
        assert_eq!(symbol_name(nid, 0, 0), "wzvqT4UqKX8#A#A");
        assert!(Nid::decode("not-eleven").is_err());
    }

    /// The fingerprint is the suffix bytes in hex, not a digest of them.
    #[test]
    fn the_fingerprint_is_the_suffix_and_not_a_digest_of_it() {
        assert_eq!(suffix_fingerprint(&[0x51, 0x8D, 0x64, 0xA6]), "518D64A6");
        assert_eq!(suffix_fingerprint(&[]), "");
    }
}
