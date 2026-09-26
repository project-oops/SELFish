//! Wrapping a key under a package public key.
//!
//! Entries `0x10` and `0x20` carry key material wrapped under RSA-2048. Writing them needs only
//! the public half of each key (D054). The block is `PKCS#1` type 2 in shape, but its filler
//! comes from a Mersenne Twister seeded from the modulus and the key, so the same inputs always
//! give the same 256 bytes and `examples/wrap_keys.rs` can compare them with a real package.
//!
//! ```text
//! seed    = SHA-256(SHA-256(modulus || key)) read as eight big-endian words
//! filler  = SHA-256 of twelve big-endian words from the twister, zeros skipped, repeated
//! block   = 00 02 <filler to 0xDF> 00 <key at 0xE0..0x100>
//! out     = block ^ 65537 mod modulus
//! ```

use num_bigint::BigUint;
use sha2::{Digest, Sha256};

/// How long a wrapped block is.
pub const BLOCK_LEN: usize = 256;
/// Where the payload sits inside the padded block.
const PAYLOAD_AT: usize = 224;
/// The zero byte separating the filler from the payload.
const SEPARATOR_AT: usize = 223;
/// Where the filler begins.
const FILLER_AT: usize = 2;
/// The public exponent, which is the usual one.
const EXPONENT: u32 = 65537;

/// Wrap a thirty-two byte key under a modulus.
///
/// `modulus` is 256 bytes big-endian; `key` is the 32 bytes to wrap.
///
/// # Errors
///
/// If either input is the wrong length.
pub fn wrap_key(modulus: &[u8], key: &[u8]) -> Result<Vec<u8>, WrapError> {
    if modulus.len() != BLOCK_LEN {
        return Err(WrapError::BadModulus(modulus.len()));
    }
    if key.len() != 32 {
        return Err(WrapError::BadKey(key.len()));
    }

    // The twister is seeded from both the modulus and the payload, so a given key wrapped
    // under a given modulus always produces the same block.
    let mut hasher = Sha256::new();
    hasher.update(modulus);
    hasher.update(key);
    let once: [u8; 32] = hasher.finalize().into();
    let seed_bytes: [u8; 32] = Sha256::digest(once).into();

    let mut seed = [0_u32; 8];
    for (index, word) in seed.iter_mut().enumerate() {
        let at = index.saturating_mul(4);
        let slice = seed_bytes.get(at..at.saturating_add(4)).unwrap_or(&[0; 4]);
        *word = u32::from_be_bytes([
            *slice.first().unwrap_or(&0),
            *slice.get(1).unwrap_or(&0),
            *slice.get(2).unwrap_or(&0),
            *slice.get(3).unwrap_or(&0),
        ]);
    }
    let mut twister = MersenneTwister::from_seed(&seed);

    let mut block = [0_u8; BLOCK_LEN];
    if let Some(byte) = block.get_mut(1) {
        *byte = 2;
    }
    if let Some(slot) = block.get_mut(PAYLOAD_AT..) {
        slot.copy_from_slice(key);
    }

    // Filler, taken a digest at a time. Zero bytes are skipped rather than used, which is what
    // makes the separator at 0xDF unambiguous.
    let mut at = FILLER_AT;
    while at < SEPARATOR_AT {
        let mut source = Vec::with_capacity(48);
        for _ in 0..12 {
            source.extend_from_slice(&twister.next_u32().to_be_bytes());
        }
        let random: [u8; 32] = Sha256::digest(&source).into();
        for byte in random {
            if at >= SEPARATOR_AT {
                break;
            }
            if byte != 0 {
                if let Some(slot) = block.get_mut(at) {
                    *slot = byte;
                }
                at = at.saturating_add(1);
            }
        }
    }

    // The public operation, which is all a wrap needs.
    let message = BigUint::from_bytes_be(&block);
    let modulus = BigUint::from_bytes_be(modulus);
    let wrapped = message.modpow(&BigUint::from(EXPONENT), &modulus);

    // Left-padded to the full width; a short big-endian encoding here would shift every byte.
    let raw = wrapped.to_bytes_be();
    if raw.len() > BLOCK_LEN {
        return Err(WrapError::Overflow);
    }
    let mut out = vec![0_u8; BLOCK_LEN];
    let at = BLOCK_LEN.saturating_sub(raw.len());
    if let Some(slot) = out.get_mut(at..) {
        slot.copy_from_slice(&raw);
    }
    Ok(out)
}

/// Why a wrap could not be produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WrapError {
    /// The modulus was not 256 bytes.
    BadModulus(usize),
    /// The key was not 32 bytes.
    BadKey(usize),
    /// The result did not fit, which means the modulus was not a 2048-bit one.
    Overflow,
}

impl core::fmt::Display for WrapError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::BadModulus(len) => write!(f, "a modulus must be 256 bytes, got {len}"),
            Self::BadKey(len) => write!(f, "a key must be 32 bytes, got {len}"),
            Self::Overflow => write!(f, "the wrapped block did not fit in 256 bytes"),
        }
    }
}

impl core::error::Error for WrapError {}

/// The Mersenne Twister the padding is drawn from.
///
/// `MT19937`, seeded by array (`init_by_array`), which differs from seeding by one word.
struct MersenneTwister {
    state: [u32; Self::N],
    index: usize,
}

impl MersenneTwister {
    const N: usize = 624;
    const M: usize = 397;
    const MATRIX_A: u32 = 0x9908_b0df;
    const UPPER_MASK: u32 = 0x8000_0000;
    const LOWER_MASK: u32 = 0x7fff_ffff;
    /// The default seed the reference implementation starts from before mixing in the array.
    const DEFAULT_SEED: u32 = 0x012B_D6AA;
    const INIT_MULTIPLIER: u32 = 0x6C07_8965;
    const MIX_ONE: u32 = 0x0019_660D;
    const MIX_TWO: u32 = 0x5D58_8B65;
    const TEMPER_ONE: u32 = 0x9d2c_5680;
    const TEMPER_TWO: u32 = 0xefc6_0000;

    fn from_word(seed: u32) -> Self {
        let mut state = [0_u32; Self::N];
        if let Some(first) = state.first_mut() {
            *first = seed;
        }
        for index in 1..Self::N {
            let previous = state.get(index.saturating_sub(1)).copied().unwrap_or(0);
            let value = (u32::try_from(index).unwrap_or(0))
                .wrapping_add(Self::INIT_MULTIPLIER.wrapping_mul(previous ^ (previous >> 30)));
            if let Some(slot) = state.get_mut(index) {
                *slot = value;
            }
        }
        Self {
            state,
            index: Self::N,
        }
    }

    fn from_seed(seed: &[u32]) -> Self {
        let mut twister = Self::from_word(Self::DEFAULT_SEED);
        let mut at = 1_usize;
        let mut from = 0_usize;

        for _ in 0..Self::N.max(seed.len()) {
            let previous = twister
                .state
                .get(at.saturating_sub(1))
                .copied()
                .unwrap_or(0);
            let current = twister.state.get(at).copied().unwrap_or(0);
            let value = (current ^ ((previous ^ (previous >> 30)).wrapping_mul(Self::MIX_ONE)))
                .wrapping_add(seed.get(from).copied().unwrap_or(0))
                .wrapping_add(u32::try_from(from).unwrap_or(0));
            if let Some(slot) = twister.state.get_mut(at) {
                *slot = value;
            }
            at = at.saturating_add(1);
            from = from.saturating_add(1);
            if at >= Self::N {
                let last = twister.state.get(Self::N - 1).copied().unwrap_or(0);
                if let Some(first) = twister.state.first_mut() {
                    *first = last;
                }
                at = 1;
            }
            if from >= seed.len() {
                from = 0;
            }
        }

        for _ in 0..Self::N.saturating_sub(1) {
            let previous = twister
                .state
                .get(at.saturating_sub(1))
                .copied()
                .unwrap_or(0);
            let current = twister.state.get(at).copied().unwrap_or(0);
            let value = (current ^ ((previous ^ (previous >> 30)).wrapping_mul(Self::MIX_TWO)))
                .wrapping_sub(u32::try_from(at).unwrap_or(0));
            if let Some(slot) = twister.state.get_mut(at) {
                *slot = value;
            }
            at = at.saturating_add(1);
            if at >= Self::N {
                let last = twister.state.get(Self::N - 1).copied().unwrap_or(0);
                if let Some(first) = twister.state.first_mut() {
                    *first = last;
                }
                at = 1;
            }
        }

        // The reference sets the top bit so the state cannot be all zero.
        if let Some(first) = twister.state.first_mut() {
            *first = 1_u32 << 31;
        }
        twister.index = Self::N;
        twister
    }

    fn next_u32(&mut self) -> u32 {
        if self.index >= Self::N {
            self.twist();
        }
        let mut y = self.state.get(self.index).copied().unwrap_or(0);
        self.index = self.index.saturating_add(1);
        y ^= y >> 11;
        y ^= (y << 7) & Self::TEMPER_ONE;
        y ^= (y << 15) & Self::TEMPER_TWO;
        y ^ (y >> 18)
    }

    fn twist(&mut self) {
        for k in 0..Self::N {
            let current = self.state.get(k).copied().unwrap_or(0);
            let next = self
                .state
                .get(k.saturating_add(1) % Self::N)
                .copied()
                .unwrap_or(0);
            let y = (current & Self::UPPER_MASK) | (next & Self::LOWER_MASK);
            let mixed = self
                .state
                .get(k.saturating_add(Self::M) % Self::N)
                .copied()
                .unwrap_or(0)
                ^ (y >> 1)
                ^ if y & 1 == 0 { 0 } else { Self::MATRIX_A };
            if let Some(slot) = self.state.get_mut(k) {
                *slot = mixed;
            }
        }
        self.index = 0;
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing, which is what a test is for"
)]
mod tests {
    use super::{BLOCK_LEN, WrapError, wrap_key};

    fn modulus() -> Vec<u8> {
        // Not a real key: an odd number of the right width is enough to exercise the shape.
        let mut out = vec![0xAB_u8; BLOCK_LEN];
        out[0] = 0xC0;
        out[BLOCK_LEN - 1] = 0x01;
        out
    }

    /// Wrapping is deterministic: the same inputs give the same 256-byte block.
    #[test]
    fn the_same_inputs_always_wrap_to_the_same_block() {
        let key = [0x5A_u8; 32];
        let first = wrap_key(&modulus(), &key).expect("a block");
        let second = wrap_key(&modulus(), &key).expect("a block");
        assert_eq!(first, second);
        assert_eq!(first.len(), BLOCK_LEN);
    }

    /// Different keys wrap to different blocks.
    #[test]
    fn a_different_key_wraps_differently() {
        let one = wrap_key(&modulus(), &[1_u8; 32]).expect("a block");
        let two = wrap_key(&modulus(), &[2_u8; 32]).expect("a block");
        assert_ne!(one, two);
    }

    /// The padding seed depends on the modulus as well as the key.
    #[test]
    fn the_padding_seed_depends_on_the_modulus_too() {
        let mut other = modulus();
        other[1] ^= 0xFF;
        assert_ne!(
            wrap_key(&modulus(), &[7_u8; 32]).expect("a block"),
            wrap_key(&other, &[7_u8; 32]).expect("a block")
        );
    }

    /// A modulus or key of the wrong length is refused.
    #[test]
    fn wrong_lengths_are_refused() {
        assert_eq!(wrap_key(&[0; 8], &[0; 32]), Err(WrapError::BadModulus(8)));
        assert_eq!(wrap_key(&modulus(), &[0; 16]), Err(WrapError::BadKey(16)));
    }
}
