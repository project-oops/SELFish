//! The `PFSC` container, written.
//!
//! # It does not compress
//!
//! The name says compression and the reader implements zlib, so the obvious assumption is that
//! a writer has to compress too. It does not. `LibOrbisPkg@6434772`'s writer says so in its own class
//! comment - it writes the header "and doesn't actually do compression or anything interesting"
//! - and the format allows it: the block map is a list of absolute offsets, and blocks laid out
//! end to end at a fixed stride are a valid map. A reader that finds a block already the full
//! block size hands it back as-is.
//!
//! So this is a header, a map, and the data unchanged. That is worth stating plainly because
//! "implement zlib compression" looked like the blocker and was not one.
//!
//! # Layout
//!
//! ```text
//! 0x000  magic, `PFSC`, big-endian
//! 0x004  unknown, zero
//! 0x008  unknown, 6 in every image examined
//! 0x00C  block size
//! 0x010  block size again, as eight bytes -- what a reader uses
//! 0x018  where the block map starts, always 0x400
//! 0x020  where the data starts, which is the header size
//! 0x028  the data length
//! 0x400  the block map: one offset per block, plus a terminator
//! ```

use crate::PfsError;

/// The magic, big-endian, at the start.
pub const MAGIC: u32 = 0x5046_5343;
/// Where the block map begins.
const MAP_AT: u64 = 0x400;
/// The smallest a header can be. It grows only if the map does not fit.
const MIN_HEADER: u64 = 0x10000;
/// How much room the map has before the header has to grow.
const MAP_ROOM: u64 = 0xFC00;
/// Offsets within the header.
mod field {
    /// Unknown, and `6` in every image examined.
    pub(super) const UNKNOWN: usize = 0x08;
    /// Block size, as four bytes.
    pub(super) const BLOCK_SIZE_32: usize = 0x0C;
    /// Block size, as eight. This is the one a reader uses.
    pub(super) const BLOCK_SIZE_64: usize = 0x10;
    /// Where the block map starts.
    pub(super) const MAP_OFFSET: usize = 0x18;
    /// Where the data starts.
    pub(super) const DATA_AT: usize = 0x20;
    /// How much data there is.
    pub(super) const DATA_LEN: usize = 0x28;
}
/// The value at [`field::UNKNOWN`], unexplained but constant.
const UNKNOWN: u64 = 6;

/// Wrap bytes in a `PFSC` container.
///
/// The payload is stored unchanged, one block per map entry.
///
/// # Errors
///
/// If `block_size` is zero, or the result does not fit the format's offsets.
pub fn wrap(payload: &[u8], block_size: u32) -> Result<Vec<u8>, PfsError> {
    let block = u64::from(block_size);
    if block == 0 {
        return Err(PfsError::Malformed("block size is zero"));
    }
    let length = u64::try_from(payload.len()).map_err(|_| PfsError::OutOfRange)?;
    let blocks = length.div_ceil(block);

    // The map holds one offset per block plus a terminator, so a reader can get a block's
    // length by subtracting from the next entry. If it does not fit under the first block, the
    // header grows by whole blocks.
    let map_bytes = blocks
        .checked_add(1)
        .and_then(|entries| entries.checked_mul(8))
        .ok_or(PfsError::OutOfRange)?;
    let header = if map_bytes <= MAP_ROOM {
        MIN_HEADER
    } else {
        let extra = map_bytes
            .checked_sub(MAP_ROOM)
            .ok_or(PfsError::OutOfRange)?
            .div_ceil(block);
        MIN_HEADER
            .checked_add(extra.checked_mul(block).ok_or(PfsError::OutOfRange)?)
            .ok_or(PfsError::OutOfRange)?
    };

    // The data is padded to whole blocks, because every map entry is one block apart.
    let data = blocks.checked_mul(block).ok_or(PfsError::OutOfRange)?;
    let total = usize::try_from(header.checked_add(data).ok_or(PfsError::OutOfRange)?)
        .map_err(|_| PfsError::OutOfRange)?;
    let mut out = vec![0_u8; total];

    put(&mut out, 0, &MAGIC.to_be_bytes())?;
    put(&mut out, field::UNKNOWN, &UNKNOWN.to_le_bytes())?;
    put(&mut out, field::BLOCK_SIZE_32, &block_size.to_le_bytes())?;
    put(&mut out, field::BLOCK_SIZE_64, &block.to_le_bytes())?;
    put(&mut out, field::MAP_OFFSET, &MAP_AT.to_le_bytes())?;
    put(&mut out, field::DATA_AT, &header.to_le_bytes())?;
    put(&mut out, field::DATA_LEN, &data.to_le_bytes())?;

    for index in 0..=blocks {
        let at = usize::try_from(
            MAP_AT
                .checked_add(index.checked_mul(8).ok_or(PfsError::OutOfRange)?)
                .ok_or(PfsError::OutOfRange)?,
        )
        .map_err(|_| PfsError::OutOfRange)?;
        let offset = header
            .checked_add(index.checked_mul(block).ok_or(PfsError::OutOfRange)?)
            .ok_or(PfsError::OutOfRange)?;
        put(&mut out, at, &offset.to_le_bytes())?;
    }

    let at = usize::try_from(header).map_err(|_| PfsError::OutOfRange)?;
    let end = at.checked_add(payload.len()).ok_or(PfsError::OutOfRange)?;
    out.get_mut(at..end)
        .ok_or(PfsError::OutOfRange)?
        .copy_from_slice(payload);
    Ok(out)
}

fn put(out: &mut [u8], at: usize, value: &[u8]) -> Result<(), PfsError> {
    let end = at.checked_add(value.len()).ok_or(PfsError::OutOfRange)?;
    out.get_mut(at..end)
        .ok_or(PfsError::OutOfRange)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing, which is what a test is for"
)]
mod tests {
    use super::{MAGIC, wrap};
    use crate::{Compressed, Slice, Source};

    const BLOCK: u32 = 0x10000;

    #[test]
    fn what_is_wrapped_is_what_comes_back_out() {
        // Two blocks and a bit, so the map has entries a reader has to follow rather than one.
        let payload: Vec<u8> = (0..140_000_u32)
            .map(|n| u8::try_from(n & 0xFF).unwrap())
            .collect();
        let wrapped = wrap(&payload, BLOCK).expect("a container");
        assert_eq!(&wrapped[..4], &MAGIC.to_be_bytes());

        let reader = Compressed::new(Slice::new(&wrapped, 0)).expect("a reader");
        let back = reader.read(0, payload.len()).expect("the payload");
        assert_eq!(back, payload);
    }

    #[test]
    fn a_read_that_starts_part_way_into_a_block_still_lands_right() {
        let payload: Vec<u8> = (0..200_000_u32)
            .map(|n| u8::try_from(n & 0xFF).unwrap())
            .collect();
        let wrapped = wrap(&payload, BLOCK).expect("a container");
        let reader = Compressed::new(Slice::new(&wrapped, 0)).expect("a reader");
        // Straddling a block boundary is where an off-by-one in the map shows up.
        let back = reader.read(0xFFF0, 0x40).expect("a slice");
        assert_eq!(back, &payload[0xFFF0..0x10030]);
    }

    #[test]
    fn a_map_too_large_for_the_first_block_grows_the_header() {
        // Over 8000 blocks puts the map past 0xFC00 and the header has to grow. Building the
        // payload for real would be half a gigabyte, so this checks the arithmetic through a
        // small block size, which drives the same branch.
        let payload = vec![0xAA_u8; 0x40000];
        let wrapped = wrap(&payload, 0x10).expect("a container");
        let reader = Compressed::new(Slice::new(&wrapped, 0)).expect("a reader");
        assert_eq!(reader.read(0, payload.len()).expect("payload"), payload);
    }

    #[test]
    fn a_zero_block_size_is_refused() {
        assert!(wrap(b"anything", 0).is_err());
    }
}
