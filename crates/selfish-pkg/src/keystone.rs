//! `sce_sys/keystone`, which is derived from the passcode rather than supplied.
//!
//! Every package carries this 96-byte file in its filesystem. It identifies nothing about the
//! title: it is two `HMAC-SHA256` operations over the passcode.
//!
//! ```text
//! header      "keystone" 02 00 01 00, then zeros, to 32 bytes
//! fingerprint HMAC-SHA256(keystone_hmac_key, passcode)
//! final       HMAC-SHA256(keystone_mac_data, header || fingerprint)
//! keystone    header || fingerprint || final          -- 96 bytes
//! ```
//!
//! `examples/keystone.rs` compares this against the file in real packages built with the fake
//! passcode.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::PackageError;

/// How long a keystone is.
pub const LEN: usize = 96;
/// How long each of its three parts is.
const PART: usize = 32;

/// The fixed head of the file: the word, a version, and zeros.
///
/// `6b657973746f6e65` is `keystone` in ASCII; `02 00 01 00` follows it in every sample.
const HEADER: [u8; PART] = [
    0x6b, 0x65, 0x79, 0x73, 0x74, 0x6f, 0x6e, 0x65, 0x02, 0x00, 0x01, 0x00, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Where the file belongs inside a package's filesystem.
pub const PATH: &str = "sce_sys/keystone";

/// Build the keystone for a passcode.
///
/// # Errors
///
/// If either MAC key cannot be read from the keyset.
pub fn create(passcode: &[u8]) -> Result<Vec<u8>, PackageError> {
    let fingerprint = mac("keystone_hmac_key_hex", passcode)?;

    let mut covered = Vec::with_capacity(PART.saturating_mul(2));
    covered.extend_from_slice(&HEADER);
    covered.extend_from_slice(&fingerprint);
    let final_mac = mac("keystone_mac_data_hex", &covered)?;

    let mut out = covered;
    out.extend_from_slice(&final_mac);
    Ok(out)
}

/// One `HMAC-SHA256` under a key named in the keyset.
fn mac(key_name: &str, message: &[u8]) -> Result<Vec<u8>, PackageError> {
    let key = crate::keys::hex_value(key_name).ok_or(PackageError::KeysUnreadable)?;
    let mut hmac =
        <Hmac<Sha256> as Mac>::new_from_slice(&key).map_err(|_| PackageError::KeysUnreadable)?;
    hmac.update(message);
    Ok(hmac.finalize().into_bytes().to_vec())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing, which is what a test is for"
)]
mod tests {
    use super::{HEADER, LEN, create};
    use crate::keys::FAKE_PASSCODE;

    /// A keystone is 96 bytes and begins with the fixed header.
    #[test]
    fn it_is_ninety_six_bytes_and_starts_with_its_name() {
        let keystone = create(FAKE_PASSCODE).expect("a keystone");
        assert_eq!(keystone.len(), LEN);
        assert_eq!(&keystone[..8], b"keystone");
        assert_eq!(&keystone[..HEADER.len()], &HEADER);
    }

    /// Different passcodes give different keystones past the shared header.
    #[test]
    fn it_follows_the_passcode() {
        let fake = create(FAKE_PASSCODE).expect("a keystone");
        let other = create(b"anotherpasscodethirtytwochars000").expect("a keystone");
        assert_ne!(fake, other);
        // ...but only past the fixed header.
        assert_eq!(&fake[..HEADER.len()], &other[..HEADER.len()]);
    }

    /// The final MAC covers the header and fingerprint, so it follows the passcode.
    #[test]
    fn the_last_third_covers_the_first_two() {
        let fake = create(FAKE_PASSCODE).expect("a keystone");
        let other = create(b"anotherpasscodethirtytwochars000").expect("a keystone");
        assert_ne!(&fake[64..], &other[64..], "the final MAC must follow too");
    }
}
