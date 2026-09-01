//! Recovering the key that unlocks a package's filesystem.
//!
//! Four steps stand between a package and a filename, and none of them can be skipped:
//!
//! ```text
//! entry 0x10, bytes 0x400..0x500  --RSA(dk3)-->  dk3
//! sha256(entry 0x20's table row || dk3)          -> key and iv
//! entry 0x20's data      --AES-CBC(key, iv)-->   wrapped image key
//! that                   --RSA(fake)-->          the filesystem key
//! ```
//!
//! # Fake packages only, and that is a wall rather than a gap
//!
//! The keyset here is the public one every open-source packaging tool uses. It unlocks a
//! package built *with* it. A retail package's image key is encrypted under a key nobody
//! outside the vendor has, and no amount of work here changes that - the extractor these
//! constants came from says as much in its own header.
//!
//! # Padding is checked, not assumed
//!
//! Both RSA steps produce a PKCS#1 v1.5 block, and the padding is validated before the
//! payload is taken. A malformed block that is silently unwrapped yields a key-shaped
//! quantity that decrypts everything to noise - which surfaces hundreds of lines later as an
//! unreadable filesystem rather than as the wrong key it is.

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::NoPadding};
use num_bigint::BigUint;
use sha2::{Digest, Sha256};

use crate::{PackageError, entry_id};

/// The committed keyset, with its provenance beside it.
const KEYS_TOML: &str = include_str!("../../../data/pkg-keys.toml");

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;

/// Size of an RSA-2048 block, in bytes.
const RSA_BLOCK: usize = 0x100;

/// Where the wrapped `dk3` lives inside the entry-keys blob.
///
/// # This was an open question and is not one any more
///
/// D044 recorded `0x400` as a measured range with the note that the key index "does not agree"
/// with it and that nothing established how one mapped onto the other. That was true of a
/// reader. It stopped being true the moment this crate could *write* the blob (D054), and the
/// comment outlived the uncertainty by several hundred lines of the same file.
///
/// The old model was "eight RSA-2048 blocks, and `0x400` is block four". The blob is not that.
/// It is a digest, seven check digests, then seven wrapped keys:
///
/// ```text
/// 0x000  SHA-256 of the padded content id
/// 0x020  7 x 32   a digest per key, so a holder can check one without revealing it
/// 0x100  7 x 256  the wrapped keys
/// ```
///
/// So key index `n` is at `0x100 + n * 256`, the index maps **directly**, and index 3 lands on
/// `0x400` - the measured range, arrived at from the other direction. Derived from
/// [`dk3_block_at`] rather than written out, so the reader and the writer cannot drift apart.
const DK3_RANGE: core::ops::Range<usize> = dk3_block_at()..dk3_block_at() + RSA_BLOCK;

/// Bit of an entry's first flags word marking it encrypted.
pub const FLAG_ENCRYPTED: u32 = 1 << 31;

/// Which key an entry was encrypted under, from bits 12-15 of its second flags word.
#[must_use]
pub const fn key_index(flags2: u32) -> u32 {
    (flags2 & 0xF000) >> 12
}

/// The AES key over a licence's secret field.
///
/// `None` if the keyset cannot be read, which is a build problem rather than a runtime one.
#[must_use]
pub fn rif_secret_key() -> Option<[u8; 16]> {
    let bytes = hex_value("rif_secret_key_hex")?;
    let mut out = [0_u8; 16];
    out.copy_from_slice(bytes.get(..16)?);
    Some(out)
}

/// Sign a PKCS#1 block with the debug RIF keyset.
///
/// One modular exponentiation, which is the whole of RSA signing. The padding is the caller's
/// because the scheme belongs to what is being signed, not to the key.
///
/// **Confirmed against real material**: with this keyset and a DigestInfo-prefixed PKCS#1 v1.5
/// block, the output is byte-identical to the stored signature of every licence in every
/// package examined. (D047)
#[must_use]
pub fn sign_debug_rif(block: &[u8]) -> Option<Vec<u8>> {
    let pair = KeyPair::load("debugrif")?;
    let value = BigUint::from_bytes_be(block);
    if value >= pair.modulus {
        return None;
    }
    let signed = value.modpow(&pair.exponent, &pair.modulus);
    let mut out = vec![0_u8; RSA_BLOCK];
    let raw = signed.to_bytes_be();
    let at = RSA_BLOCK.checked_sub(raw.len())?;
    out.get_mut(at..)?.copy_from_slice(&raw);
    Some(out)
}

/// One RSA keypair from the data file.
#[derive(Debug)]
struct KeyPair {
    modulus: BigUint,
    exponent: BigUint,
}

impl KeyPair {
    fn load(prefix: &str) -> Option<Self> {
        Some(Self {
            modulus: read_key(&format!("{prefix}_modulus_hex"))?,
            exponent: read_key(&format!("{prefix}_exponent_hex"))?,
        })
    }

    /// One modular exponentiation, then the padding is stripped.
    fn unwrap_block(&self, block: &[u8]) -> Option<Vec<u8>> {
        let plain = BigUint::from_bytes_be(block)
            .modpow(&self.exponent, &self.modulus)
            .to_bytes_be();
        // `to_bytes_be` drops leading zeros, and PKCS#1 v1.5 begins with one. Restored to a
        // fixed width so the marker bytes are where the format says rather than wherever the
        // value happened to land.
        let mut padded = vec![0_u8; RSA_BLOCK.saturating_sub(plain.len())];
        padded.extend_from_slice(&plain);
        strip_pkcs1(&padded)
    }
}

/// Strip PKCS#1 v1.5 padding: `00 02 <nonzero...> 00 <payload>`.
///
/// Returns `None` for anything else. The alternative - taking whatever follows and hoping -
/// produces a key-shaped quantity that decrypts everything to noise, and the failure then
/// appears hundreds of lines away as a corrupt filesystem.
fn strip_pkcs1(block: &[u8]) -> Option<Vec<u8>> {
    if block.first() != Some(&0x00) || block.get(1) != Some(&0x02) {
        return None;
    }
    let separator = block.iter().skip(2).position(|b| *b == 0x00)?;
    let start = separator.checked_add(3)?;
    Some(block.get(start..)?.to_vec())
}

/// Find the assignment for a key, and return everything after the `=`.
///
/// # Why this is not `split_once(name)`
///
/// It was, and that made the keyset file's **prose** part of its parser. Adding a table to the
/// header that named each key - `| rif_secret_key_hex | decrypts ... |` - put that string a
/// hundred lines above its own assignment, so the reader matched the comment, took the next
/// quoted text out of it, and handed back a key of the wrong length. Every licence test failed,
/// and nothing about the change looked like code.
///
/// A key is only a key where it is assigned: at the start of a line, followed by `=`. Comments
/// may now say any name they like. (D068)
fn assignment(name: &str) -> Option<&'static str> {
    KEYS_TOML.lines().find_map(|line| {
        let rest = line.strip_prefix(name)?.trim_start();
        rest.strip_prefix('=')
    })
}

/// A quoted single-line value from the keyset file.
fn read_value(name: &str) -> Option<String> {
    let body = assignment(name)?.split_once('"')?.1.split_once('"')?.0;
    Some(body.chars().filter(char::is_ascii_hexdigit).collect())
}

/// One single-line hex value from the keyset, as bytes.
///
/// `read_value` finds the text; this is the step that turns it into bytes. Shared because more
/// than one key in that file is a fixed-width byte string rather than a bignum.
pub(crate) fn hex_value(name: &str) -> Option<Vec<u8>> {
    let text = read_value(name)?;
    if text.is_empty() || text.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks_exact(2) {
        out.push(u8::from_str_radix(core::str::from_utf8(pair).ok()?, 16).ok()?);
    }
    Some(out)
}

fn read_key(name: &str) -> Option<BigUint> {
    // The multi-line values start on the assignment line and run to the closing fence, so the
    // rest of the file from that point is the right span to search. Same reasoning as
    // `assignment`: a name in a comment is not an assignment.
    let at = KEYS_TOML.find(&format!("\n{name}"))?;
    let after = KEYS_TOML.get(at..)?;
    let body = after.split_once("\"\"\"")?.1.split_once("\"\"\"")?.0;
    let hex: String = body.chars().filter(char::is_ascii_hexdigit).collect();
    if hex.is_empty() {
        return None;
    }
    BigUint::parse_bytes(hex.as_bytes(), 16)
}

/// Recover the filesystem key from a parsed package.
///
/// # Errors
///
/// If either key entry is absent or truncated, if a padding check fails, or if the keyset
/// could not be read. A padding failure most often means the package is retail rather than
/// fake, which is not a bug to fix.
pub fn filesystem_key(package: &crate::Package<'_>) -> Result<Vec<u8>, PackageError> {
    let dk3_pair = KeyPair::load("dk3").ok_or(PackageError::KeysUnreadable)?;
    let fake_pair = KeyPair::load("fake").ok_or(PackageError::KeysUnreadable)?;

    let entry_keys = package
        .entry(entry_id::ENTRY_KEYS)
        .ok_or(PackageError::MissingEntry(entry_id::ENTRY_KEYS))?;
    let image_key = package
        .entry(entry_id::IMAGE_KEY)
        .ok_or(PackageError::MissingEntry(entry_id::IMAGE_KEY))?;

    let entry_keys_bytes = package
        .entry_bytes(entry_keys)
        .ok_or(PackageError::EntryTruncated(entry_id::ENTRY_KEYS))?;
    let wrapped_dk3 = entry_keys_bytes
        .get(DK3_RANGE)
        .ok_or(PackageError::EntryTruncated(entry_id::ENTRY_KEYS))?;
    let dk3 = dk3_pair
        .unwrap_block(wrapped_dk3)
        .ok_or(PackageError::NotAFakePackage)?;

    // The image key entry's own **table row** is hashed, not its data. Two different
    // thirty-two-byte quantities associated with one entry, and using the wrong one produces
    // a key that is exactly as plausible and entirely wrong.
    let row = package
        .entry_row(image_key)
        .ok_or(PackageError::EntryTruncated(entry_id::IMAGE_KEY))?;
    let mut hasher = Sha256::new();
    hasher.update(row);
    hasher.update(&dk3);
    let derived = hasher.finalize();

    let iv = derived.get(..16).ok_or(PackageError::KeysUnreadable)?;
    let key = derived.get(16..32).ok_or(PackageError::KeysUnreadable)?;

    let mut buffer = package
        .entry_bytes(image_key)
        .ok_or(PackageError::EntryTruncated(entry_id::IMAGE_KEY))?
        .to_vec();
    let decryptor =
        Aes128CbcDec::new_from_slices(key, iv).map_err(|_| PackageError::KeysUnreadable)?;
    let decrypted = decryptor
        .decrypt_padded_mut::<NoPadding>(&mut buffer)
        .map_err(|_| PackageError::NotAFakePackage)?
        .to_vec();

    fake_pair
        .unwrap_block(&decrypted)
        .ok_or(PackageError::NotAFakePackage)
}

/// Decrypt an entry that declares itself encrypted.
///
/// # The derivation is the one the image key already uses
///
/// An entry's **table row** is hashed with the unwrapped key material, and the digest splits
/// into an IV and an AES-128 key. That is exactly what [`filesystem_key`] does for the image
/// key, and the reason `entry_row` exists separately from `entry_bytes`: two different
/// thirty-two-byte quantities belong to one entry, and using the wrong one produces a key that
/// is exactly as plausible and entirely wrong.
///
/// # What this establishes and what it does not
///
/// Entries under the index the image key declares are decrypted from the key blob, which needs
/// no passcode and works on any package. Anything else falls through to
/// [`decrypt_entry_with_passcode`] with the community default, which is right for a package
/// built with it and wrong - visibly, as noise - for one that was not.
///
/// # Errors
///
/// If the entry is not marked encrypted, or if the keyset or key material cannot be read.
pub fn decrypt_entry(
    package: &crate::Package<'_>,
    entry: &crate::Entry,
) -> Result<Vec<u8>, PackageError> {
    if entry.flags1 & FLAG_ENCRYPTED == 0 {
        return Err(PackageError::NotEncrypted(entry.id));
    }
    // Two routes reach an encrypted entry and they are not equivalent.
    //
    // The **key blob** carries one unwrapped block, and the image key declares its index - so
    // that route works for entries under that index, in any package, with no passcode. It
    // reaches nothing else.
    //
    // The **computed** route derives keying material from the content id, the passcode and the
    // index, so it reaches every index - but only if the passcode is the one the package was
    // built with. Of three packages examined, two use the community default and one does not.
    //
    // Blob first because it needs no guess, then computed. Where both reach the same entry
    // they produce identical bytes, which is checked by `examples/decrypt`. (D044)
    // Blob first because it needs no guess, then computed. The fall-through is on *failure*
    // rather than only on a different index: a package whose key blob this crate cannot unwrap
    // is not necessarily one whose entries it cannot read, and refusing on the first route
    // would have hidden that.
    let index = key_index(entry.flags2);
    if index != IMAGE_KEY_INDEX {
        return decrypt_entry_with_passcode(package, entry, FAKE_PASSCODE);
    }
    match decrypt_entry_from_blob(package, entry) {
        Ok(plain) => Ok(plain),
        Err(_) => decrypt_entry_with_passcode(package, entry, FAKE_PASSCODE),
    }
}

/// The key-blob route, which needs no passcode but reaches only one key index.
fn decrypt_entry_from_blob(
    package: &crate::Package<'_>,
    entry: &crate::Entry,
) -> Result<Vec<u8>, PackageError> {
    let dk3_pair = KeyPair::load("dk3").ok_or(PackageError::KeysUnreadable)?;
    let entry_keys = package
        .entry(entry_id::ENTRY_KEYS)
        .ok_or(PackageError::MissingEntry(entry_id::ENTRY_KEYS))?;
    let wrapped = package
        .entry_bytes(entry_keys)
        .and_then(|bytes| bytes.get(DK3_RANGE))
        .ok_or(PackageError::EntryTruncated(entry_id::ENTRY_KEYS))?;
    let dk3 = dk3_pair
        .unwrap_block(wrapped)
        .ok_or(PackageError::NotAFakePackage)?;

    let row = package
        .entry_row(entry)
        .ok_or(PackageError::EntryTruncated(entry.id))?;
    let mut hasher = Sha256::new();
    hasher.update(row);
    hasher.update(&dk3);
    let derived = hasher.finalize();

    let iv = derived.get(..16).ok_or(PackageError::KeysUnreadable)?;
    let key = derived.get(16..32).ok_or(PackageError::KeysUnreadable)?;

    let mut buffer = package
        .entry_bytes(entry)
        .ok_or(PackageError::EntryTruncated(entry.id))?
        .to_vec();
    let decryptor =
        Aes128CbcDec::new_from_slices(key, iv).map_err(|_| PackageError::KeysUnreadable)?;
    Ok(decryptor
        .decrypt_padded_mut::<NoPadding>(&mut buffer)
        .map_err(|_| PackageError::NotAFakePackage)?
        .to_vec())
}

/// How long a content id is.
pub const CONTENT_ID_LEN: usize = 36;
/// How long a passcode is.
pub const PASSCODE_LEN: usize = 32;
/// The passcode a fake package is built with.
///
/// Thirty-two ASCII zeros. Community tooling uses this because a fake package has no real
/// entitlement behind it, and it is confirmed here rather than assumed: with it, the licence
/// entry decrypts to `RIF` in every package examined, and the result is byte-identical to what
/// the entirely separate key route produces for the same entry. (D044)
pub const FAKE_PASSCODE: &[u8; PASSCODE_LEN] = b"00000000000000000000000000000000";

/// The key index the filesystem key uses.
///
/// The same derivation the entry keys use, at index 1. `LibOrbisPkg/PKG/PkgBuilder.cs` reaches
/// it as `ComputeKeys(ContentId, Passcode, 1)`.
pub const FILESYSTEM_KEY_INDEX: u32 = 1;

/// The key the filesystem inside a package is signed and encrypted with - `EKPFS`.
///
/// # Nothing here is recovered from anything
///
/// This is worth stating because the surrounding work spent a long time on keys that genuinely
/// cannot be obtained. This one is not like those. It is a hash of the content id and the
/// passcode, both of which a builder chooses, so it is *computed* rather than found. From it,
/// [`selfish_pfs::outer::sign_key`] and [`selfish_pfs::outer::encryption_keys`] derive
/// everything the filesystem layer needs.
///
/// A caller building a fake package passes [`FAKE_PASSCODE`].
///
/// # The same key, reached from the other end
///
/// [`filesystem_key`] recovers this from a package that already exists, by decrypting the image
/// key entry with the fake keyset. This computes it for a package that does not exist yet. For
/// any fake package the two must agree, which makes the pair an oracle rather than two
/// functions that happen to share a name - see the test beside them.
#[must_use]
pub fn derive_filesystem_key(content_id: &[u8], passcode: &[u8]) -> [u8; 32] {
    compute_keys(content_id, passcode, FILESYSTEM_KEY_INDEX)
}

/// The keying material for one key index.
///
/// Three thirty-two-byte pieces hashed together: the digest of the index, the digest of the
/// content id padded to 48 bytes, and the passcode verbatim. `LibOrbisPkg/Util/Crypto.cs`.
#[must_use]
pub fn compute_keys(content_id: &[u8], passcode: &[u8], index: u32) -> [u8; 32] {
    let mut buffer = [0_u8; 96];

    let mut hasher = Sha256::new();
    hasher.update(index.to_be_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    if let Some(slot) = buffer.get_mut(..32) {
        slot.copy_from_slice(&digest);
    }

    // Padded to forty-eight with NULs, which is longer than the id itself - the padding is
    // part of what is hashed, so trimming it produces a different key.
    let mut padded = [0_u8; 48];
    let take = content_id.len().min(padded.len());
    if let (Some(into), Some(from)) = (padded.get_mut(..take), content_id.get(..take)) {
        into.copy_from_slice(from);
    }
    let mut hasher = Sha256::new();
    hasher.update(padded);
    let digest: [u8; 32] = hasher.finalize().into();
    if let Some(slot) = buffer.get_mut(32..64) {
        slot.copy_from_slice(&digest);
    }

    let take = passcode.len().min(PASSCODE_LEN);
    if let (Some(into), Some(from)) = (
        buffer.get_mut(64..64usize.saturating_add(take)),
        passcode.get(..take),
    ) {
        into.copy_from_slice(from);
    }

    let mut hasher = Sha256::new();
    hasher.update(buffer);
    hasher.finalize().into()
}

/// Decrypt an entry using keying material computed from the content id and passcode.
///
/// The general route, and the one that reaches every key index. [`decrypt_entry`] is the
/// special case this crate had first: it works for the index the image key declares because
/// the key blob happens to carry that block, and reaches nothing else.
///
/// # Errors
///
/// If the entry is not marked encrypted, or its data or table row cannot be read.
pub fn decrypt_entry_with_passcode(
    package: &crate::Package<'_>,
    entry: &crate::Entry,
    passcode: &[u8],
) -> Result<Vec<u8>, PackageError> {
    if entry.flags1 & FLAG_ENCRYPTED == 0 {
        return Err(PackageError::NotEncrypted(entry.id));
    }
    let material = compute_keys(package.content_id(), passcode, key_index(entry.flags2));

    // The entry's **table row**, as everywhere else in this derivation.
    let row = package
        .entry_row(entry)
        .ok_or(PackageError::EntryTruncated(entry.id))?;
    let mut hasher = Sha256::new();
    hasher.update(row);
    hasher.update(material);
    let derived = hasher.finalize();

    let iv = derived.get(..16).ok_or(PackageError::KeysUnreadable)?;
    let key = derived.get(16..32).ok_or(PackageError::KeysUnreadable)?;
    let mut buffer = package
        .entry_bytes(entry)
        .ok_or(PackageError::EntryTruncated(entry.id))?
        .to_vec();
    let decryptor =
        Aes128CbcDec::new_from_slices(key, iv).map_err(|_| PackageError::KeysUnreadable)?;
    Ok(decryptor
        .decrypt_padded_mut::<NoPadding>(&mut buffer)
        .map_err(|_| PackageError::NotAFakePackage)?
        .to_vec())
}

/// Encrypt an entry body the way a package stores it.
///
/// The inverse of [`decrypt_entry_with_passcode`], and it takes the entry's table **row**
/// rather than the entry itself because a writer computes the row before it has anything to
/// look up - the row is the input to the derivation, not a consequence of it.
///
/// # Errors
///
/// If the cipher rejects the derived key, which cannot happen for a 32-byte digest.
pub fn encrypt_body(
    row: &[u8],
    content_id: &[u8],
    passcode: &[u8],
    index: u32,
    body: &mut [u8],
) -> Result<(), PackageError> {
    let material = compute_keys(content_id, passcode, index);
    let mut hasher = Sha256::new();
    hasher.update(row);
    hasher.update(material);
    let derived = hasher.finalize();

    let iv = derived.get(..16).ok_or(PackageError::KeysUnreadable)?;
    let key = derived.get(16..32).ok_or(PackageError::KeysUnreadable)?;
    let encryptor =
        Aes128CbcEnc::new_from_slices(key, iv).map_err(|_| PackageError::KeysUnreadable)?;
    let len = body.len();
    encryptor
        .encrypt_padded_mut::<NoPadding>(body, len)
        .map_err(|_| PackageError::KeysUnreadable)?;
    Ok(())
}

/// Try every block of the key entry against one encrypted entry.
///
/// A search, not a derivation. It exists because an entry can declare a key index this crate
/// cannot yet map onto the key blob, and the honest way to close that is to try each block and
/// look at what comes out - a block that yields a recognisable structure is the answer, and one
/// that yields noise says only that it was the wrong block.
///
/// Returns every attempt so a caller can judge them, rather than picking a winner here. Nothing
/// in this crate decides a key is right because its output looked plausible.
#[must_use]
pub fn decrypt_entry_with_each_key(
    package: &crate::Package<'_>,
    entry: &crate::Entry,
) -> Vec<(usize, Vec<u8>)> {
    let Some(pair) = KeyPair::load("dk3") else {
        return Vec::new();
    };
    let Some(blob) = package
        .entry(entry_id::ENTRY_KEYS)
        .and_then(|keys| package.entry_bytes(keys))
    else {
        return Vec::new();
    };
    let Some(row) = package.entry_row(entry) else {
        return Vec::new();
    };
    let Some(data) = package.entry_bytes(entry) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for block in 0..blob.len().checked_div(RSA_BLOCK).unwrap_or(0) {
        let at = block.saturating_mul(RSA_BLOCK);
        let Some(wrapped) = blob.get(at..at.saturating_add(RSA_BLOCK)) else {
            continue;
        };
        let Some(unwrapped) = pair.unwrap_block(wrapped) else {
            continue;
        };
        let mut hasher = Sha256::new();
        hasher.update(row);
        hasher.update(&unwrapped);
        let derived = hasher.finalize();
        let (Some(iv), Some(key)) = (derived.get(..16), derived.get(16..32)) else {
            continue;
        };
        let mut buffer = data.to_vec();
        let Ok(decryptor) = Aes128CbcDec::new_from_slices(key, iv) else {
            continue;
        };
        if let Ok(plain) = decryptor.decrypt_padded_mut::<NoPadding>(&mut buffer) {
            out.push((block, plain.to_vec()));
        }
    }
    out
}

/// The key index the image key declares, and the only one the key blob carries.
pub const IMAGE_KEY_INDEX: u32 = 3;

/// How many key indices a package's key blob carries.
pub const KEY_INDICES: usize = 7;
/// How long the entry-keys blob at `0x10` is.
pub const ENTRY_KEYS_LEN: usize = 2048;
/// Where the wrapped keys begin inside that blob.
const WRAPPED_AT: usize = 0x100;

/// One of the seven package public moduli, as 256 big-endian bytes.
#[must_use]
pub fn pkg_public_modulus(index: usize) -> Option<Vec<u8>> {
    to_block(&read_key(&format!("pkg_public_{index}_hex"))?)
}

/// Left-pad a value to a full RSA block.
///
/// `to_bytes_be` drops leading zeros, and a modulus that happens to start with one would
/// otherwise shift every byte of the block that is built from it.
fn to_block(value: &BigUint) -> Option<Vec<u8>> {
    let raw = value.to_bytes_be();
    let at = RSA_BLOCK.checked_sub(raw.len())?;
    let mut out = vec![0_u8; at];
    out.extend_from_slice(&raw);
    Some(out)
}

/// Build the entry-keys blob a package carries at `0x10`.
///
/// ```text
/// 0x000  SHA-256(content id padded to 48)
/// 0x020  7 x  SHA-256(key[i]) XOR key[i]
/// 0x100  7 x  the key for index i, wrapped under public key i
/// ```
///
/// Index 3's wrapped block therefore lands at `0x400`, which is exactly the range
/// [`filesystem_key`] reads from a real package. The reading side was written first, from
/// measurement, and the two agree without either having been adjusted to suit the other.
///
/// **Index 0 wraps the passcode itself**, not the key derived from it. That is not an oddity
/// to tidy away: it is how a console recovers a passcode it was never told.
///
/// # Errors
///
/// If the passcode is the wrong length, a public modulus cannot be read, or a wrap fails.
pub fn entry_keys_blob(content_id: &[u8], passcode: &[u8]) -> Result<Vec<u8>, PackageError> {
    if passcode.len() != PASSCODE_LEN {
        return Err(PackageError::KeysUnreadable);
    }
    let mut out = Vec::with_capacity(ENTRY_KEYS_LEN);

    let mut padded = [0_u8; 48];
    let take = content_id.len().min(padded.len());
    if let (Some(into), Some(from)) = (padded.get_mut(..take), content_id.get(..take)) {
        into.copy_from_slice(from);
    }
    out.extend_from_slice(&Sha256::digest(padded));

    let mut wrapped = Vec::with_capacity(KEY_INDICES);
    for index in 0..KEY_INDICES {
        let key = compute_keys(content_id, passcode, u32::try_from(index).unwrap_or(0));
        // A digest that lets a holder of the key confirm it without the key being present.
        let digest: [u8; 32] = Sha256::digest(key).into();
        let mut check = [0_u8; 32];
        for (slot, (left, right)) in check.iter_mut().zip(digest.iter().zip(key.iter())) {
            *slot = left ^ right;
        }
        out.extend_from_slice(&check);

        let modulus = pkg_public_modulus(index).ok_or(PackageError::KeysUnreadable)?;
        let payload: &[u8] = if index == 0 { passcode } else { &key };
        wrapped.push(
            crate::wrap::wrap_key(&modulus, payload).map_err(|_| PackageError::KeysUnreadable)?,
        );
    }
    for block in wrapped {
        out.extend_from_slice(&block);
    }

    if out.len() != ENTRY_KEYS_LEN {
        return Err(PackageError::KeysUnreadable);
    }
    Ok(out)
}

/// Build the image-key blob a package carries at `0x20`, before the entry is encrypted.
///
/// The filesystem key, wrapped under the fake keyset. This is what a console unwraps in order
/// to reach the filesystem at all, and **a package carrying zeros here cannot be opened** -
/// which is what this crate emitted for as long as it took the blob as an input.
///
/// The result is the entry's *plaintext*. The entry is stored AES-encrypted under key index 3,
/// which the builder applies as it does to any other encrypted entry.
///
/// # Errors
///
/// If the fake modulus cannot be read, or the wrap fails.
pub fn image_key_blob(content_id: &[u8], passcode: &[u8]) -> Result<Vec<u8>, PackageError> {
    let modulus = to_block(&read_key("fake_modulus_hex").ok_or(PackageError::KeysUnreadable)?)
        .ok_or(PackageError::KeysUnreadable)?;
    let ekpfs = derive_filesystem_key(content_id, passcode);
    crate::wrap::wrap_key(&modulus, &ekpfs).map_err(|_| PackageError::KeysUnreadable)
}

/// Where index 3's wrapped block sits inside the entry-keys blob.
///
/// Two separately-derived numbers meet here: this is `WRAPPED_AT + 3 * RSA_BLOCK`, and it is
/// also the range the reader measured from real packages long before any of this was written.
#[must_use]
pub const fn dk3_block_at() -> usize {
    WRAPPED_AT + 3 * RSA_BLOCK
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{KeyPair, RSA_BLOCK, strip_pkcs1};

    #[test]
    fn both_keypairs_load_from_the_data_file() {
        assert!(KeyPair::load("fake").is_some(), "the fake keypair");
        assert!(KeyPair::load("dk3").is_some(), "the dk3 keypair");
        assert!(KeyPair::load("nonexistent").is_none());
    }

    #[test]
    fn the_moduli_are_the_size_the_format_requires() {
        // A key that parsed but came out the wrong size would still exponentiate, and every
        // block it produced would be wrong in a way nothing downstream could attribute.
        let fake = KeyPair::load("fake").expect("fake");
        let dk3 = KeyPair::load("dk3").expect("dk3");
        assert_eq!(fake.modulus.bits(), 2048, "fake modulus");
        assert_eq!(dk3.modulus.bits(), 2048, "dk3 modulus");
        assert!(fake.exponent.bits() > 2000, "fake exponent");
        assert!(dk3.exponent.bits() > 2000, "dk3 exponent");
    }

    #[test]
    fn padding_is_stripped_only_when_it_is_actually_there() {
        let mut block = vec![0_u8; RSA_BLOCK];
        block[1] = 0x02;
        for (index, byte) in block.iter_mut().enumerate().take(20).skip(2) {
            *byte = u8::try_from(index).unwrap_or(0xFF);
        }
        block[20] = 0x00;
        block[21] = 0xAB;
        assert_eq!(strip_pkcs1(&block).map(|p| p.len()), Some(RSA_BLOCK - 21));
    }

    #[test]
    fn a_block_without_the_marker_is_refused_rather_than_guessed_at() {
        // The important half. An unwrapped-but-malformed block yields a key-shaped quantity
        // that decrypts everything to noise, and the failure appears far from its cause.
        let mut wrong_first = vec![0_u8; RSA_BLOCK];
        wrong_first[0] = 0x01;
        wrong_first[1] = 0x02;
        assert_eq!(strip_pkcs1(&wrong_first), None);

        let mut wrong_second = vec![0_u8; RSA_BLOCK];
        wrong_second[1] = 0x01;
        assert_eq!(strip_pkcs1(&wrong_second), None);

        // Correct markers but no separator, so there is no payload boundary to find.
        let mut no_separator = vec![0xFF_u8; RSA_BLOCK];
        no_separator[0] = 0x00;
        no_separator[1] = 0x02;
        assert_eq!(strip_pkcs1(&no_separator), None);

        assert_eq!(strip_pkcs1(&[]), None);
    }
}
