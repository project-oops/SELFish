//! `sce_sys/keystone`, which is derived rather than supplied.
//!
//! # Why this is a format product and not title content
//!
//! Every real package carries a 96-byte `keystone` in its filesystem, and a builder was
//! demanding it as an input because it looked like one more file the caller owns. It is not:
//! it is two `HMAC-SHA256` operations over the **passcode**, which the builder already has.
//! Nothing about it identifies a title.
//!
//! It was missing from every package this crate produced, and every real package has one - the
//! kind of difference that is invisible until an installer refuses.
//!
//! ```text
//! header      "keystone" 02 00 01 00, then zeros, to 32 bytes
//! fingerprint HMAC-SHA256(keystone_hmac_key, passcode)
//! final       HMAC-SHA256(keystone_mac_data, header || fingerprint)
//! keystone    header || fingerprint || final          -- 96 bytes
//! ```
//!
//! # Confirmed against real packages
//!
//! `examples/keystone.rs` extracts the `keystone` from packages in hand and compares. Two of
//! the three to hand were built with the fake passcode and both reproduce **byte for byte**;
//! the third used a passcode nobody can recover, so it differs and is reported as such rather
//! than as a failure.

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
/// If either MAC key cannot be read from the keyset, which is a build problem rather than a
/// runtime one.
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

    #[test]
    fn it_is_ninety_six_bytes_and_starts_with_its_name() {
        // Every real package's keystone is exactly this long. A different length means the
        // construction changed, not that a value did.
        let keystone = create(FAKE_PASSCODE).expect("a keystone");
        assert_eq!(keystone.len(), LEN);
        assert_eq!(&keystone[..8], b"keystone");
        assert_eq!(&keystone[..HEADER.len()], &HEADER);
    }

    #[test]
    fn it_follows_the_passcode() {
        // The whole point: it is derived from the passcode, so two packages keyed differently
        // carry different keystones. If this ever stops being true the derivation is broken.
        let fake = create(FAKE_PASSCODE).expect("a keystone");
        let other = create(b"anotherpasscodethirtytwochars000").expect("a keystone");
        assert_ne!(fake, other);
        // ...but only past the fixed header.
        assert_eq!(&fake[..HEADER.len()], &other[..HEADER.len()]);
    }

    #[test]
    fn the_last_third_covers_the_first_two() {
        // The final MAC is taken over the header *and* the fingerprint, so changing the
        // passcode has to move it too. Computing it over the fingerprint alone would be an
        // easy simplification to make and would produce a file that looks right.
        let fake = create(FAKE_PASSCODE).expect("a keystone");
        let other = create(b"anotherpasscodethirtytwochars000").expect("a keystone");
        assert_ne!(&fake[64..], &other[64..], "the final MAC must follow too");
    }
}
