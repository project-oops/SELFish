//! The licence a package carries, read and written.
//!
//! Entry `0x400` is a RIF - 1024 bytes, big-endian, ending in an RSA-2048 signature. Entry
//! `0x401` is a shorter record naming the same content. Both are stored encrypted; see
//! [`crate::keys::decrypt_entry`] for how they come out.
//!
//! The offsets are measured from real licences. A published field list places the disc key at
//! `0x1E8` and the signature at `0x2A8`, which cannot fit: the populated region is
//! `0x260..0x400` and the signature ends on the structure's last byte. [`Licence::build`]
//! reproduces a real licence byte for byte from its content id and passcode.

use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::NoPadding};
use sha2::{Digest, Sha256};

use crate::{PackageError, keys};

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;

/// How long a licence is.
pub const SIZE: usize = 0x400;
/// How long the shorter licence record is.
pub const INFO_SIZE: usize = 0x200;

/// Offsets within a licence, measured from real material.
pub mod field {
    /// `RIF\0`.
    pub const MAGIC: usize = 0x00;
    /// Format version.
    pub const VERSION: usize = 0x04;
    /// Unnamed, `0xFFFF` in every sample.
    pub const UNKNOWN: usize = 0x06;
    /// Account id. Zero for a fake licence.
    pub const ACCOUNT_ID: usize = 0x08;
    /// Start of validity.
    pub const START_TIME: usize = 0x10;
    /// End of validity.
    pub const END_TIME: usize = 0x18;
    /// The content id, NUL padded.
    pub const CONTENT_ID: usize = 0x20;
    /// Licence type.
    pub const LICENSE_TYPE: usize = 0x50;
    /// DRM type.
    pub const DRM_TYPE: usize = 0x52;
    /// Content type.
    pub const CONTENT_TYPE: usize = 0x54;
    /// SKU flag.
    pub const SKU_FLAG: usize = 0x56;
    /// Flags.
    pub const FLAGS: usize = 0x58;
    /// Unnamed, `1` in every sample.
    pub const UNK_64: usize = 0x64;
    /// Disc key. Zero in every sample.
    pub const DISC_KEY: usize = 0x240;
    /// The IV over the secret.
    pub const SECRET_IV: usize = 0x260;
    /// The secret, encrypted.
    pub const SECRET: usize = 0x270;
    /// The signature, over everything before it.
    pub const SIGNATURE: usize = 0x300;
}

/// Constants a fake licence declares.
pub mod value {
    /// The magic.
    pub const MAGIC: [u8; 4] = *b"RIF\0";
    /// Format version.
    pub const VERSION: u16 = 1;
    /// The unnamed field before the account id.
    pub const UNKNOWN: u16 = 0xFFFF;
    /// Start of validity, fixed in every sample and in the source that writes them.
    pub const START_TIME: u64 = 1_364_222_275;
    /// A debug licence, which is what a fake package carries.
    pub const LICENSE_TYPE: u16 = 0x200;
    /// The value at [`super::field::UNK_64`].
    pub const UNK_64: u32 = 1;
}

/// How long the secret is.
const SECRET_LEN: usize = 144;
/// How long a signature is.
const SIGNATURE_LEN: usize = 256;
/// The `DigestInfo` prefix for a SHA-256 signature, per PKCS#1.
///
/// With it, signing a real licence reproduces its stored signature exactly.
const DIGEST_INFO: [u8; 19] = [
    0x30, 0x31, 0x30, 0x0D, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05,
    0x00, 0x04, 0x20,
];

/// A licence, in the clear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Licence {
    /// The bytes, all `0x400` of them.
    pub bytes: Vec<u8>,
}

impl Licence {
    /// Build one for a content id.
    ///
    /// `drm_type`, `content_type` and `sku_flag` describe the title and come from the caller.
    ///
    /// # Errors
    ///
    /// If the keyset cannot be read.
    pub fn build(
        content_id: &[u8],
        drm_type: u16,
        content_type: u16,
        sku_flag: u16,
    ) -> Result<Self, PackageError> {
        let mut out = vec![0_u8; SIZE];

        put(&mut out, field::MAGIC, &value::MAGIC);
        put(&mut out, field::VERSION, &value::VERSION.to_be_bytes());
        put(&mut out, field::UNKNOWN, &value::UNKNOWN.to_be_bytes());
        put(
            &mut out,
            field::START_TIME,
            &value::START_TIME.to_be_bytes(),
        );
        put(&mut out, field::END_TIME, &i64::MAX.to_be_bytes());
        put(&mut out, field::CONTENT_ID, content_id);
        put(
            &mut out,
            field::LICENSE_TYPE,
            &value::LICENSE_TYPE.to_be_bytes(),
        );
        put(&mut out, field::DRM_TYPE, &drm_type.to_be_bytes());
        put(&mut out, field::CONTENT_TYPE, &content_type.to_be_bytes());
        put(&mut out, field::SKU_FLAG, &sku_flag.to_be_bytes());
        put(&mut out, field::UNK_64, &value::UNK_64.to_be_bytes());

        // The account id, flags and disc key are zero in every sample and are left unwritten.

        // The IV and the first half of the secret are the two halves of one digest over the
        // padded content id.
        let digest = padded_content_digest(content_id);
        put(&mut out, field::SECRET_IV, digest.get(..16).unwrap_or(&[]));

        let mut secret = vec![0_u8; SECRET_LEN];
        put(&mut secret, 0, digest.get(16..32).unwrap_or(&[]));
        let iv: [u8; 16] = digest
            .get(..16)
            .and_then(|slice| slice.try_into().ok())
            .unwrap_or([0; 16]);
        encrypt_secret(&mut secret, &iv)?;
        put(&mut out, field::SECRET, &secret);

        let signature = sign(out.get(..field::SIGNATURE).unwrap_or_default())?;
        put(&mut out, field::SIGNATURE, &signature);

        Ok(Self { bytes: out })
    }

    /// The shorter licence record, entry `0x401`.
    ///
    /// The content id, then zeros, to a fixed length, as in every sample.
    #[must_use]
    pub fn info(content_id: &[u8]) -> Vec<u8> {
        let mut out = vec![0_u8; INFO_SIZE];
        put(&mut out, 0, content_id);
        out
    }

    /// Whether the signature is the one this keyset would produce.
    ///
    /// # Errors
    ///
    /// If the keyset cannot be read.
    pub fn signature_is_valid(&self) -> Result<bool, PackageError> {
        let over = self.bytes.get(..field::SIGNATURE).unwrap_or_default();
        let stored = self
            .bytes
            .get(field::SIGNATURE..field::SIGNATURE.saturating_add(SIGNATURE_LEN))
            .unwrap_or_default();
        Ok(sign(over)? == stored)
    }
}

/// SHA-256 of the content id padded to 48 bytes with NULs.
///
/// The padding is part of what is hashed.
fn padded_content_digest(content_id: &[u8]) -> [u8; 32] {
    let mut padded = [0_u8; 48];
    let take = content_id.len().min(padded.len());
    if let (Some(into), Some(from)) = (padded.get_mut(..take), content_id.get(..take)) {
        into.copy_from_slice(from);
    }
    Sha256::digest(padded).into()
}

/// AES-128-CBC over the secret, in place.
///
/// The source routine is named `AesCbcCfb128Encrypt` but sets `CipherMode.CBC`, and real
/// licences agree with CBC.
fn encrypt_secret(secret: &mut [u8], iv: &[u8; 16]) -> Result<(), PackageError> {
    let key = keys::rif_secret_key().ok_or(PackageError::KeysUnreadable)?;
    let encryptor = Aes128CbcEnc::new_from_slices(&key, iv).map_err(|_| PackageError::BadKey)?;
    let len = secret.len();
    encryptor
        .encrypt_padded_mut::<NoPadding>(secret, len)
        .map_err(|_| PackageError::BadKey)?;
    Ok(())
}

/// Sign a digest of `over` with the debug RIF keyset.
///
/// PKCS#1 v1.5 with a `DigestInfo` prefix, which reproduces real signatures.
fn sign(over: &[u8]) -> Result<Vec<u8>, PackageError> {
    let digest: [u8; 32] = Sha256::digest(over).into();

    let mut block = vec![0xFF_u8; SIGNATURE_LEN];
    put(&mut block, 0, &[0x00, 0x01]);
    let tail = SIGNATURE_LEN
        .checked_sub(DIGEST_INFO.len().saturating_add(digest.len()))
        .ok_or(PackageError::KeysUnreadable)?;
    put(&mut block, tail.saturating_sub(1), &[0x00]);
    put(&mut block, tail, &DIGEST_INFO);
    put(&mut block, tail.saturating_add(DIGEST_INFO.len()), &digest);

    keys::sign_debug_rif(&block).ok_or(PackageError::KeysUnreadable)
}

fn put(out: &mut [u8], at: usize, bytes: &[u8]) {
    let end = at.saturating_add(bytes.len());
    if let Some(slot) = out.get_mut(at..end) {
        slot.copy_from_slice(bytes);
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{Licence, SIZE, field, value};

    const ID: &[u8] = b"IV0002-ITEM00001_00-STOREUPD00000000";

    /// A built licence has the measured size, magic, content id and zero regions.
    #[test]
    fn a_built_licence_has_the_shape_the_measurements_describe() {
        let licence = Licence::build(ID, 0, 0, 0).expect("a licence");
        assert_eq!(licence.bytes.len(), SIZE);
        assert_eq!(&licence.bytes[..4], b"RIF\0");
        assert_eq!(
            &licence.bytes[field::CONTENT_ID..field::CONTENT_ID + ID.len()],
            ID
        );
        assert!(
            licence.bytes[field::DISC_KEY..field::DISC_KEY + 32]
                .iter()
                .all(|byte| *byte == 0),
            "the disc key is zero in every real licence"
        );
        assert!(
            licence.bytes[0x6C..field::DISC_KEY]
                .iter()
                .all(|byte| *byte == 0),
            "and so is everything between the header and it"
        );
    }

    /// The start time is the fixed value real licences carry.
    #[test]
    fn the_start_time_is_the_one_every_real_licence_carries() {
        let licence = Licence::build(ID, 0, 0, 0).expect("a licence");
        let mut raw = [0_u8; 8];
        raw.copy_from_slice(&licence.bytes[field::START_TIME..field::START_TIME + 8]);
        assert_eq!(u64::from_be_bytes(raw), value::START_TIME);
    }

    /// A built licence's signature verifies against the keyset.
    #[test]
    fn a_licence_this_crate_builds_verifies_against_the_keyset() {
        let licence = Licence::build(ID, 0, 0, 0).expect("a licence");
        assert!(licence.signature_is_valid().expect("a keyset"));
    }

    /// Changing one signed byte invalidates the signature.
    #[test]
    fn changing_one_byte_invalidates_the_signature() {
        let mut licence = Licence::build(ID, 0, 0, 0).expect("a licence");
        licence.bytes[field::CONTENT_ID] ^= 1;
        assert!(
            !licence.signature_is_valid().expect("a keyset"),
            "a signature that survives its content changing is not a signature"
        );
    }

    /// The info record is the content id followed by zeros.
    #[test]
    fn the_info_record_is_the_content_id_then_zeros() {
        let info = Licence::info(ID);
        assert_eq!(info.len(), super::INFO_SIZE);
        assert_eq!(&info[..ID.len()], ID);
        assert!(info[ID.len()..].iter().all(|byte| *byte == 0));
    }
}
