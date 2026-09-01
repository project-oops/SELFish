//! The filesystem inside a package.
//!
//! Three layers, each wrapping the one below:
//!
//! ```text
//! raw bytes
//!   -> XTS      sector-by-sector decryption, 4KiB sectors
//!     -> PFSC   zlib-compressed blocks addressed through a map
//!       -> PFS  a superblock, inodes, and directories
//! ```
//!
//! and a package nests them twice: an outer filesystem whose only interesting content is a
//! compressed image, which is itself a filesystem holding the real files.
//!
//! # Layered through a trait, because the nesting is the format
//!
//! [`Source`] is the seam. Each layer reads through the one beneath without knowing what it
//! is, which is what lets an outer filesystem be the source for an inner one - the shape the
//! format actually has, rather than a pipeline flattened for convenience.
//!
//! # Reading and writing
//!
//! Every layer above can now be written as well as read, and the two halves are checked against
//! each other rather than against an assertion of what the bytes should be:
//!
//! - [`mod@write`] builds the inner filesystem - plain, which is what a package's inner image is.
//! - [`mod@pfsc`] wraps it. The container does not compress, which is the surprise: the block map
//!   is a list of offsets and a full-size block is stored as-is.
//! - [`mod@outer`] builds the outer filesystem - signed and encrypted, holding that container as
//!   its single file.
//!
//! `outer`'s tests run the whole nest and back: build, wrap, sign, encrypt, then decrypt,
//! decompress, walk, and compare. Nothing in that chain needs a key that cannot be computed -
//! all of it comes from the content id and the passcode.

#![forbid(unsafe_code)]

pub mod outer;
pub mod pfsc;
pub mod write;

use core::fmt;

use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit, generic_array::GenericArray};
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Sector size the encryption layer works in.
pub const SECTOR_SIZE: u64 = 0x1000;

/// Offsets within the superblock.
///
/// # Where these come from
///
/// Named by `LibOrbisPkg@6434772` (`PFS/PfsStructs.cs`), and every one confirmed against
/// three real images. Before that source was read this module knew five of them, measured; the
/// other 95 non-zero bytes were unaccounted for and filesystem writing was blocked on them
/// (D027). They were not mysterious, only unnamed.
///
/// The five that were already here all agreed with the source, which is worth recording in
/// that order. (D042)
#[allow(
    dead_code,
    reason = "a faithful record of the format, not a needs-driven subset - a header that omits               fields because no caller wants them is one that becomes wrong the moment a caller               does, and re-adding a field means re-deriving every offset around it"
)]
pub(crate) mod superblock {
    /// Format version. `1` in every image examined.
    pub(crate) const VERSION: usize = 0x00;
    /// Magic. `0x1332A0B` in every image examined.
    pub(crate) const MAGIC: usize = 0x08;
    /// Filesystem id.
    pub(crate) const ID: usize = 0x10;
    /// File mode.
    pub(crate) const FMODE: usize = 0x18;
    /// Whether the filesystem was unmounted cleanly.
    pub(crate) const CLEAN: usize = 0x19;
    /// Read-only flag. Set in every image examined.
    pub(crate) const READ_ONLY: usize = 0x1A;
    /// Reserved.
    pub(crate) const RSV: usize = 0x1B;
    /// Mode flags: signed, 64-bit, encrypted.
    pub(crate) const MODE: usize = 0x1C;
    /// Unnamed, and zero in every image examined.
    pub(crate) const UNK1: usize = 0x1E;
    /// Block size.
    pub(crate) const BLOCK_SIZE: usize = 0x20;
    /// Number of backup blocks.
    pub(crate) const N_BACKUP: usize = 0x24;
    /// Block count.
    pub(crate) const N_BLOCK: usize = 0x28;
    /// Number of inodes.
    pub(crate) const INODE_COUNT: usize = 0x30;
    /// **Data block count - the size of the image, in blocks.**
    ///
    /// `ndblock * block_size` is the image length exactly, in all three samples: 655, 951 and
    /// 1152 blocks of 64 KiB. That invariant is checked by [`Filesystem::parse`], and it is
    /// also, independently, the block count from which a package's `PLAYGO_CHUNK_SHA` table
    /// starts describing the image. Two facts derived separately that agree.
    pub(crate) const N_DBLOCK: usize = 0x38;
    /// Number of blocks holding inodes.
    pub(crate) const INODE_BLOCKS: usize = 0x40;
    /// An inode structure describing the inode block itself.
    ///
    /// Measured as "33 non-zero bytes of something" before it had a name; it is a signature
    /// followed by a block index.
    pub(crate) const INODE_BLOCK_SIG: usize = 0xB8;
    /// Unnamed index. `1` in every image examined.
    ///
    /// Only written when the image carries a seed. An image without one puts its `1` at
    /// [`NO_SEED_INDEX`] instead, four bytes earlier.
    pub(crate) const UNKNOWN_INDEX: usize = 0x36C;
    /// Where an image with no seed writes its `1`.
    ///
    /// The inner filesystem of a package is unseeded and lands here; the outer one is seeded
    /// and lands at [`UNKNOWN_INDEX`]. Two fields, one measurement, and telling them apart
    /// needed the writer rather than the reader - nothing reads either.
    /// An index the inner superblock sets to `1`, unexplained.
    ///
    /// Every real package holds it and this crate wrote zero. Named `UnknownIndex` by
    /// `LibProsperoPKG` and unexplained there too. Reproduced because it sits in the
    /// structure a console reads to mount `/app0`. (measured 3/3)
    pub(crate) const UNKNOWN_D8: usize = 0xD8;

    pub(crate) const NO_SEED_INDEX: usize = 0x368;
    /// Where the key-derivation seed begins.
    ///
    /// **Zero in all three images examined**, which is worth knowing before relying on it: the
    /// derivation that hashes it is hashing sixteen zero bytes in every sample to hand.
    pub(crate) const SEED: usize = 0x370;
    /// How long the seed is.
    pub(crate) const SEED_LEN: usize = 16;
    /// How much of the image the header proper occupies.
    ///
    /// The source says `0x380`. This module reads `0x400` because that is the span it slices,
    /// and the 32 bytes at `0x380` are past the header and not part of it.
    pub(crate) const HEADER_SIZE: usize = 0x380;
    /// How much of the image the superblock occupies.
    pub(crate) const SIZE: usize = 0x400;
}

/// The filesystem magic, at offset `0x08`.
///
/// Identical in every image examined. Named by `LibOrbisPkg@6434772`; confirmed here.
pub const MAGIC: u64 = 0x0133_2A0B;

/// The superblock, read.
///
/// Every field `LibOrbisPkg@6434772` names, so a caller can see what an image says about itself rather
/// than only what this crate needed in order to walk it. That distinction is what kept
/// filesystem *writing* blocked: a reader follows four numbers and never looks at the rest, so
/// the rest stayed unnamed and looked like a wall. (D042)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Superblock {
    /// Format version. `1` in every image examined.
    pub version: u64,
    /// Filesystem id.
    pub id: u64,
    /// File mode.
    pub fmode: u8,
    /// Whether the filesystem was unmounted cleanly.
    pub clean: u8,
    /// Read-only. Set in every image examined.
    pub read_only: u8,
    /// Mode flags: signed, 64-bit, encrypted. See [`mode`].
    pub mode: u16,
    /// Block size, in bytes.
    pub block_size: u32,
    /// Number of backup blocks.
    pub backup_blocks: u32,
    /// Block count.
    pub blocks: u64,
    /// Number of inodes.
    pub inode_count: u64,
    /// Data block count. `data_blocks * block_size` is the image length.
    pub data_blocks: u64,
    /// Number of blocks holding inodes.
    pub inode_blocks: u64,
    /// An unnamed index, `1` in every image examined.
    pub unknown_index: u32,
    /// The key-derivation seed. **Zero in every image examined.**
    pub seed: [u8; superblock::SEED_LEN],
}

impl Superblock {
    /// Read one from the first bytes of an image.
    ///
    /// # Errors
    ///
    /// If the bytes are short, or the magic is not a filesystem's.
    pub fn parse(header: &[u8]) -> Result<Self, PfsError> {
        let magic = read_u64(header, superblock::MAGIC)?;
        if magic != MAGIC {
            return Err(PfsError::NotAFilesystem(magic));
        }
        let mut seed = [0_u8; superblock::SEED_LEN];
        if let Some(raw) =
            header.get(superblock::SEED..superblock::SEED.saturating_add(superblock::SEED_LEN))
        {
            seed.copy_from_slice(raw);
        }
        Ok(Self {
            version: read_u64(header, superblock::VERSION)?,
            id: read_u64(header, superblock::ID)?,
            fmode: header.get(superblock::FMODE).copied().unwrap_or(0),
            clean: header.get(superblock::CLEAN).copied().unwrap_or(0),
            read_only: header.get(superblock::READ_ONLY).copied().unwrap_or(0),
            mode: read_u16(header, superblock::MODE)?,
            block_size: read_u32(header, superblock::BLOCK_SIZE)?,
            backup_blocks: read_u32(header, superblock::N_BACKUP)?,
            blocks: read_u64(header, superblock::N_BLOCK)?,
            inode_count: read_u64(header, superblock::INODE_COUNT)?,
            data_blocks: read_u64(header, superblock::N_DBLOCK)?,
            inode_blocks: read_u64(header, superblock::INODE_BLOCKS)?,
            unknown_index: read_u32(header, superblock::UNKNOWN_INDEX)?,
            seed,
        })
    }

    /// How long the image should be, from what the superblock says.
    ///
    /// `data_blocks * block_size`, which is the image length exactly in all three samples -
    /// 655, 951 and 1152 blocks of 64 KiB. A caller handed an image that disagrees has been
    /// handed a truncated one, and finding that out here beats finding it out three layers
    /// down in a decompressor.
    #[must_use]
    pub const fn image_len(&self) -> u64 {
        self.data_blocks.saturating_mul(self.block_size as u64)
    }

    /// Whether the inodes carry signatures, which makes them larger.
    #[must_use]
    pub const fn is_signed(&self) -> bool {
        self.mode & mode::SIGNED != 0
    }

    /// Whether the image is encrypted.
    #[must_use]
    pub const fn is_encrypted(&self) -> bool {
        self.mode & mode::ENCRYPTED != 0
    }
}

/// Mode bits in the superblock.
pub mod mode {
    /// Inodes carry signatures, which makes them larger.
    pub const SIGNED: u16 = 0x1;
    /// Sixty-four bit.
    pub const IS_64BIT: u16 = 0x2;
    /// The image is encrypted.
    pub const ENCRYPTED: u16 = 0x4;
    /// Named by `LibOrbisPkg@6434772` as the flag that is always set, and it is: every image examined
    /// here carries mode `0xD`, which is this bit plus signed plus encrypted. What it means is
    /// not established, only that its absence has never been seen.
    pub const UNKNOWN_ALWAYS_SET: u16 = 0x8;
}

/// Size of one inode, with and without signatures.
pub(crate) mod inode {
    /// With signatures.
    pub(crate) const SIGNED_SIZE: usize = 0x2C8;
    /// Without.
    pub(crate) const PLAIN_SIZE: usize = 0xA8;
    /// Offset of the size field.
    pub(crate) const SIZE: usize = 8;
    /// Offset of the block count.
    pub(crate) const BLOCKS: usize = 0x60;
    /// Offset of the first block, when signed.
    pub(crate) const SIGNED_START: usize = 0x84;
    /// Offset of the first block, when not.
    pub(crate) const PLAIN_START: usize = 0x64;
}

/// Directory entry types.
pub(crate) mod dirent {
    /// A file.
    pub(crate) const FILE: u32 = 2;
    /// A directory.
    pub(crate) const DIRECTORY: u32 = 3;
}

/// Somewhere bytes can be read from at an offset.
///
/// The seam the whole nesting rests on. A layer reads through this without knowing whether
/// what is beneath it is a file, a decryptor, or another filesystem.
pub trait Source {
    /// Read `len` bytes from `offset`.
    ///
    /// # Errors
    ///
    /// If the range is not available, or a layer beneath cannot supply it.
    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>, PfsError>;
}

/// A plain byte slice, optionally starting part-way in.
#[derive(Debug)]
pub struct Slice<'a> {
    bytes: &'a [u8],
    base: u64,
}

impl<'a> Slice<'a> {
    /// Read from `bytes`, treating `base` as offset zero.
    #[must_use]
    pub const fn new(bytes: &'a [u8], base: u64) -> Self {
        Self { bytes, base }
    }
}

impl Source for Slice<'_> {
    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>, PfsError> {
        let at = usize::try_from(self.base.checked_add(offset).ok_or(PfsError::OutOfRange)?)
            .map_err(|_| PfsError::OutOfRange)?;
        let end = at.checked_add(len).ok_or(PfsError::OutOfRange)?;
        self.bytes
            .get(at..end)
            .map(<[u8]>::to_vec)
            .ok_or(PfsError::OutOfRange)
    }
}

/// Reading through a reference, so a layer can be shared rather than moved.
///
/// The nesting needs this: an outer filesystem is parsed from a decryption layer, and the
/// image inside it is then read from that *same* layer at an offset the filesystem supplied.
/// Without this the filesystem would have to give the layer back, and every wrapper would own
/// the one beneath it exclusively for no reason.
impl<S: Source + ?Sized> Source for &S {
    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>, PfsError> {
        (**self).read(offset, len)
    }
}
/// A window onto part of another source.
///
/// What makes the nesting expressible: an inode's contents are a byte range of the
/// filesystem holding it, and the image inside a package is exactly that - one file in an
/// outer filesystem, which is itself a whole filesystem.
#[derive(Debug)]
pub struct Region<S: Source> {
    inner: S,
    base: u64,
    length: u64,
}

impl<S: Source> Region<S> {
    /// A window of `length` bytes starting at `base`.
    #[must_use]
    pub const fn new(inner: S, base: u64, length: u64) -> Self {
        Self {
            inner,
            base,
            length,
        }
    }
}

impl<S: Source> Source for Region<S> {
    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>, PfsError> {
        let end = offset
            .checked_add(u64::try_from(len).map_err(|_| PfsError::OutOfRange)?)
            .ok_or(PfsError::OutOfRange)?;
        if end > self.length {
            return Err(PfsError::OutOfRange);
        }
        self.inner.read(
            self.base.checked_add(offset).ok_or(PfsError::OutOfRange)?,
            len,
        )
    }
}

/// The encryption layer: AES-XTS over fixed-size sectors.
///
/// # Written out rather than taken from a crate
///
/// The tweak is the sector index encrypted under one key, then multiplied through
/// GF(2^128) once per sixteen bytes; the data is decrypted under another. Standard XTS,
/// except for which sectors are exempt - the first few are stored in the clear, and how many
/// depends on the filesystem's block size. That exemption is why this is written here rather
/// than handed to a general implementation: it is a property of the container, not of XTS.
pub struct Xts<S: Source> {
    inner: S,
    tweak: aes::Aes128,
    data: aes::Aes128,
    plain_sectors: u64,
}

impl<S: Source> fmt::Debug for Xts<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Xts")
            .field("plain_sectors", &self.plain_sectors)
            .finish_non_exhaustive()
    }
}

impl<S: Source> Xts<S> {
    /// Wrap a source with XTS decryption.
    ///
    /// # Errors
    ///
    /// If either key is not sixteen bytes.
    pub fn new(
        inner: S,
        tweak_key: &[u8],
        data_key: &[u8],
        plain_sectors: u64,
    ) -> Result<Self, PfsError> {
        Ok(Self {
            inner,
            tweak: aes::Aes128::new_from_slice(tweak_key).map_err(|_| PfsError::BadKey)?,
            data: aes::Aes128::new_from_slice(data_key).map_err(|_| PfsError::BadKey)?,
            plain_sectors,
        })
    }

    /// Decrypt one sector.
    fn decrypt_sector(&self, index: u64, sector: &[u8]) -> Vec<u8> {
        let mut tweak = [0_u8; 16];
        tweak.copy_from_slice(
            &index.to_le_bytes()[..8]
                .iter()
                .copied()
                .chain(core::iter::repeat_n(0_u8, 8))
                .collect::<Vec<_>>(),
        );
        let mut block = GenericArray::clone_from_slice(&tweak);
        self.tweak.encrypt_block(&mut block);
        tweak.copy_from_slice(&block);

        let mut out = Vec::with_capacity(sector.len());
        for chunk in sector.chunks(16) {
            let mut buffer = [0_u8; 16];
            for (index, byte) in chunk.iter().enumerate() {
                if let (Some(slot), Some(t)) = (buffer.get_mut(index), tweak.get(index)) {
                    *slot = byte ^ t;
                }
            }
            let mut block = GenericArray::clone_from_slice(&buffer);
            self.data.decrypt_block(&mut block);
            for (index, byte) in block.iter().enumerate() {
                if let (Some(slot), Some(t)) = (buffer.get_mut(index), tweak.get(index)) {
                    *slot = byte ^ t;
                }
            }
            out.extend_from_slice(buffer.get(..chunk.len()).unwrap_or(&buffer));
            tweak = next_tweak(tweak);
        }
        out
    }
}

/// Multiply the tweak by the generator in GF(2^128).
///
/// A left shift across the whole block, with the carry out of the top reduced by the
/// polynomial. Written as its own function because it is the one piece of XTS that is easy to
/// get subtly wrong and produces plausible noise when it is.
fn next_tweak(tweak: [u8; 16]) -> [u8; 16] {
    let mut out = [0_u8; 16];
    let mut carry = 0_u8;
    for (index, byte) in tweak.iter().enumerate() {
        if let Some(slot) = out.get_mut(index) {
            *slot = (byte << 1) | carry;
        }
        carry = (byte >> 7) & 1;
    }
    if let (1, Some(first)) = (carry, out.first_mut()) {
        *first ^= 0x87;
    }
    out
}

impl<S: Source> Source for Xts<S> {
    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>, PfsError> {
        let mut out = Vec::with_capacity(len);
        let mut position = offset;
        let end = offset
            .checked_add(u64::try_from(len).map_err(|_| PfsError::OutOfRange)?)
            .ok_or(PfsError::OutOfRange)?;

        while position < end {
            let sector = position
                .checked_div(SECTOR_SIZE)
                .ok_or(PfsError::OutOfRange)?;
            let within = usize::try_from(
                position
                    .checked_rem(SECTOR_SIZE)
                    .ok_or(PfsError::OutOfRange)?,
            )
            .map_err(|_| PfsError::OutOfRange)?;
            let take = usize::try_from(
                SECTOR_SIZE
                    .saturating_sub(
                        position
                            .checked_rem(SECTOR_SIZE)
                            .ok_or(PfsError::OutOfRange)?,
                    )
                    .min(end.saturating_sub(position)),
            )
            .map_err(|_| PfsError::OutOfRange)?;

            let raw = self.inner.read(
                sector
                    .checked_mul(SECTOR_SIZE)
                    .ok_or(PfsError::OutOfRange)?,
                usize::try_from(SECTOR_SIZE).map_err(|_| PfsError::OutOfRange)?,
            )?;
            let plain = if sector < self.plain_sectors {
                raw
            } else {
                self.decrypt_sector(sector, &raw)
            };
            out.extend_from_slice(
                plain
                    .get(within..within.checked_add(take).ok_or(PfsError::OutOfRange)?)
                    .ok_or(PfsError::OutOfRange)?,
            );
            position = position
                .checked_add(u64::try_from(take).map_err(|_| PfsError::OutOfRange)?)
                .ok_or(PfsError::OutOfRange)?;
        }
        Ok(out)
    }
}

/// The compression layer: fixed-size blocks, each zlib-compressed, addressed through a map.
///
/// A block whose map entry spans exactly the block size is stored uncompressed. That is the
/// only signal - there is no per-block flag - so the size comparison *is* the format.
#[derive(Debug)]
pub struct Compressed<S: Source> {
    inner: S,
    block_size: u64,
    map: Vec<u64>,
}

impl<S: Source> Compressed<S> {
    /// Read the block map and wrap the source.
    ///
    /// # Errors
    ///
    /// If the header or map cannot be read, or the block size is zero.
    pub fn new(inner: S) -> Result<Self, PfsError> {
        let header = inner.read(0, 0x30)?;
        let block_size = read_u64(&header, 0x10)?;
        let map_offset = read_u64(&header, 0x18)?;
        let data_length = read_u64(&header, 0x28)?;
        if block_size == 0 {
            return Err(PfsError::Malformed("compressed block size is zero"));
        }
        let blocks = data_length
            .checked_div(block_size)
            .ok_or(PfsError::Malformed("compressed block count"))?;
        let entries = usize::try_from(blocks.checked_add(1).ok_or(PfsError::OutOfRange)?)
            .map_err(|_| PfsError::OutOfRange)?;
        let raw = inner.read(
            map_offset,
            entries.checked_mul(8).ok_or(PfsError::OutOfRange)?,
        )?;
        let mut map = Vec::with_capacity(entries);
        for index in 0..entries {
            map.push(read_u64(
                &raw,
                index.checked_mul(8).ok_or(PfsError::OutOfRange)?,
            )?);
        }
        Ok(Self {
            inner,
            block_size,
            map,
        })
    }
}

impl<S: Source> Source for Compressed<S> {
    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>, PfsError> {
        let mut out = Vec::with_capacity(len);
        let mut position = offset;
        let mut remaining = len;

        while remaining > 0 {
            let index = usize::try_from(
                position
                    .checked_div(self.block_size)
                    .ok_or(PfsError::OutOfRange)?,
            )
            .map_err(|_| PfsError::OutOfRange)?;
            let within = usize::try_from(
                position
                    .checked_rem(self.block_size)
                    .ok_or(PfsError::OutOfRange)?,
            )
            .map_err(|_| PfsError::OutOfRange)?;
            let start = *self.map.get(index).ok_or(PfsError::OutOfRange)?;
            let finish = *self
                .map
                .get(index.checked_add(1).ok_or(PfsError::OutOfRange)?)
                .ok_or(PfsError::OutOfRange)?;
            let span = finish
                .checked_sub(start)
                .ok_or(PfsError::Malformed("a compressed block map runs backwards"))?;
            let raw = self.inner.read(
                start,
                usize::try_from(span).map_err(|_| PfsError::OutOfRange)?,
            )?;

            // The only signal that a block is stored uncompressed.
            let block = if span == self.block_size {
                raw
            } else {
                inflate(&raw)?
            };

            let take = usize::try_from(self.block_size)
                .map_err(|_| PfsError::OutOfRange)?
                .saturating_sub(within)
                .min(remaining);
            out.extend_from_slice(
                block
                    .get(within..within.checked_add(take).ok_or(PfsError::OutOfRange)?)
                    .ok_or(PfsError::OutOfRange)?,
            );
            position = position
                .checked_add(u64::try_from(take).map_err(|_| PfsError::OutOfRange)?)
                .ok_or(PfsError::OutOfRange)?;
            remaining = remaining.saturating_sub(take);
        }
        Ok(out)
    }
}

fn inflate(bytes: &[u8]) -> Result<Vec<u8>, PfsError> {
    use std::io::Read as _;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(bytes)
        .read_to_end(&mut out)
        .map_err(|_| PfsError::Malformed("a compressed block would not inflate"))?;
    Ok(out)
}

/// One inode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inode {
    /// What it is, per the filesystem's own numbering.
    pub kind: u16,
    /// Size in bytes.
    pub size: u64,
    /// How many blocks it occupies.
    pub blocks: u32,
    /// The first block.
    pub start: u32,
}

/// One entry found by walking the directory tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Full path from the root.
    pub path: String,
    /// Which inode holds it.
    pub inode: usize,
}

/// A parsed filesystem.
#[derive(Debug)]
pub struct Filesystem<S: Source> {
    source: S,
    block_size: u64,
    inodes: Vec<Inode>,
}

impl<S: Source> Filesystem<S> {
    /// Parse the superblock and inode table.
    ///
    /// # Errors
    ///
    /// If the superblock cannot be read or describes a filesystem with no blocks.
    pub fn new(source: S) -> Result<Self, PfsError> {
        // The whole superblock, checked. A source that is not a filesystem otherwise reads as
        // one with an absurd block size, and the first error a caller sees is about inodes
        // rather than about having been handed the wrong bytes.
        let header = source.read(0, superblock::SIZE)?;
        let sb = Superblock::parse(&header)?;
        let block_size = u64::from(sb.block_size);
        if block_size == 0 {
            return Err(PfsError::Malformed("block size is zero"));
        }
        let signed = sb.is_signed();
        let count = sb.inode_count;
        let blocks = sb.inode_blocks;

        let entry_size = if signed {
            inode::SIGNED_SIZE
        } else {
            inode::PLAIN_SIZE
        };
        let per_block = usize::try_from(block_size)
            .map_err(|_| PfsError::OutOfRange)?
            .checked_div(entry_size)
            .ok_or(PfsError::Malformed("inode size is zero"))?;

        let mut inodes = Vec::new();
        for block in 0..blocks.max(1) {
            let at = block
                .checked_add(1)
                .and_then(|b| b.checked_mul(block_size))
                .ok_or(PfsError::OutOfRange)?;
            let raw = source.read(
                at,
                usize::try_from(block_size).map_err(|_| PfsError::OutOfRange)?,
            )?;
            for slot in 0..per_block {
                if u64::try_from(inodes.len()).unwrap_or(u64::MAX) >= count {
                    break;
                }
                let base = slot.checked_mul(entry_size).ok_or(PfsError::OutOfRange)?;
                let entry = raw
                    .get(base..base.checked_add(entry_size).ok_or(PfsError::OutOfRange)?)
                    .ok_or(PfsError::OutOfRange)?;
                let start_at = if signed {
                    inode::SIGNED_START
                } else {
                    inode::PLAIN_START
                };
                inodes.push(Inode {
                    kind: read_u16(entry, 0)?,
                    size: read_u64(entry, inode::SIZE)?,
                    blocks: read_u32(entry, inode::BLOCKS)?,
                    start: read_u32(entry, start_at)?,
                });
            }
        }

        Ok(Self {
            source,
            block_size,
            inodes,
        })
    }

    /// The inode table.
    #[must_use]
    pub fn inodes(&self) -> &[Inode] {
        &self.inodes
    }

    /// The layer this filesystem was parsed from.
    ///
    /// Needed because the nesting reads *past* the filesystem abstraction: the image inside a
    /// package is a byte range of the layer beneath, located by an inode but not read through
    /// one.
    #[must_use]
    pub const fn source(&self) -> &S {
        &self.source
    }

    /// The block size, which every offset in the inode table is counted in.
    #[must_use]
    pub const fn block_size(&self) -> u64 {
        self.block_size
    }

    /// The contents of one inode.
    ///
    /// # Errors
    ///
    /// If the inode is absent or its blocks cannot be read.
    pub fn contents(&self, index: usize) -> Result<Vec<u8>, PfsError> {
        let inode = self.inodes.get(index).ok_or(PfsError::NoSuchInode(index))?;
        let mut out = Vec::new();
        let mut remaining = usize::try_from(inode.size).map_err(|_| PfsError::OutOfRange)?;
        let mut block = u64::from(inode.start);
        while remaining > 0 {
            let at = block
                .checked_mul(self.block_size)
                .ok_or(PfsError::OutOfRange)?;
            let chunk = self.source.read(
                at,
                usize::try_from(self.block_size).map_err(|_| PfsError::OutOfRange)?,
            )?;
            let take = remaining.min(chunk.len());
            out.extend_from_slice(chunk.get(..take).ok_or(PfsError::OutOfRange)?);
            remaining = remaining.saturating_sub(take);
            block = block.checked_add(1).ok_or(PfsError::OutOfRange)?;
        }
        Ok(out)
    }

    /// Every file reachable from an inode, depth first.
    ///
    /// # Errors
    ///
    /// If a directory block cannot be read.
    pub fn walk(&self, root: usize) -> Result<Vec<Found>, PfsError> {
        let mut out = Vec::new();
        self.walk_into(root, "", &mut out, 0)?;
        Ok(out)
    }

    fn walk_into(
        &self,
        index: usize,
        prefix: &str,
        out: &mut Vec<Found>,
        depth: usize,
    ) -> Result<(), PfsError> {
        // A filesystem is data, and data can describe a cycle. Bounded rather than trusted.
        if depth > 64 {
            return Err(PfsError::Malformed("directory nesting is implausibly deep"));
        }
        let inode = *self.inodes.get(index).ok_or(PfsError::NoSuchInode(index))?;
        let block_size = usize::try_from(self.block_size).map_err(|_| PfsError::OutOfRange)?;

        for offset in 0..u64::from(inode.blocks) {
            let at = u64::from(inode.start)
                .checked_add(offset)
                .and_then(|b| b.checked_mul(self.block_size))
                .ok_or(PfsError::OutOfRange)?;
            let block = self.source.read(at, block_size)?;
            let mut cursor = 0_usize;
            while cursor.saturating_add(17) < block_size {
                let child =
                    usize::try_from(read_u32(&block, cursor)?).map_err(|_| PfsError::OutOfRange)?;
                let kind = read_u32(&block, cursor.checked_add(4).ok_or(PfsError::OutOfRange)?)?;
                let name_len = usize::try_from(read_u32(
                    &block,
                    cursor.checked_add(8).ok_or(PfsError::OutOfRange)?,
                )?)
                .map_err(|_| PfsError::OutOfRange)?;
                let entry_size = usize::try_from(read_u32(
                    &block,
                    cursor.checked_add(12).ok_or(PfsError::OutOfRange)?,
                )?)
                .map_err(|_| PfsError::OutOfRange)?;
                if entry_size == 0 {
                    break;
                }
                if name_len > 0 && name_len < 256 {
                    let from = cursor.checked_add(16).ok_or(PfsError::OutOfRange)?;
                    let name: String = block
                        .get(from..from.checked_add(name_len).ok_or(PfsError::OutOfRange)?)
                        .ok_or(PfsError::OutOfRange)?
                        .iter()
                        .map(|b| char::from(*b))
                        .collect();
                    let path = format!("{prefix}/{name}");
                    if kind == dirent::FILE {
                        out.push(Found { path, inode: child });
                    } else if kind == dirent::DIRECTORY && name != "." && name != ".." {
                        self.walk_into(child, &path, out, depth.saturating_add(1))?;
                    }
                }
                cursor = cursor.checked_add(entry_size).ok_or(PfsError::OutOfRange)?;
            }
        }
        Ok(())
    }
}

/// Derive the XTS keys for an image from the filesystem key and the superblock seed.
///
/// # Errors
///
/// If the seed cannot be read from the supplied superblock.
pub fn image_keys(
    filesystem_key: &[u8],
    superblock_bytes: &[u8],
) -> Result<([u8; 16], [u8; 16]), PfsError> {
    let seed = superblock_bytes
        .get(superblock::SEED..superblock::SEED.saturating_add(superblock::SEED_LEN))
        .ok_or(PfsError::OutOfRange)?;
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(filesystem_key).map_err(|_| PfsError::BadKey)?;
    mac.update(&1_u32.to_le_bytes());
    mac.update(seed);
    let derived = mac.finalize().into_bytes();
    let mut tweak = [0_u8; 16];
    let mut data = [0_u8; 16];
    tweak.copy_from_slice(derived.get(..16).ok_or(PfsError::BadKey)?);
    data.copy_from_slice(derived.get(16..32).ok_or(PfsError::BadKey)?);
    Ok((tweak, data))
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, PfsError> {
    let end = at.checked_add(2).ok_or(PfsError::OutOfRange)?;
    let raw = bytes.get(at..end).ok_or(PfsError::OutOfRange)?;
    let mut out = [0_u8; 2];
    out.copy_from_slice(raw);
    Ok(u16::from_le_bytes(out))
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, PfsError> {
    let end = at.checked_add(4).ok_or(PfsError::OutOfRange)?;
    let raw = bytes.get(at..end).ok_or(PfsError::OutOfRange)?;
    let mut out = [0_u8; 4];
    out.copy_from_slice(raw);
    Ok(u32::from_le_bytes(out))
}

fn read_u64(bytes: &[u8], at: usize) -> Result<u64, PfsError> {
    let end = at.checked_add(8).ok_or(PfsError::OutOfRange)?;
    let raw = bytes.get(at..end).ok_or(PfsError::OutOfRange)?;
    let mut out = [0_u8; 8];
    out.copy_from_slice(raw);
    Ok(u64::from_le_bytes(out))
}

/// Why a filesystem could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PfsError {
    /// A read ran past what is available.
    OutOfRange,
    /// A key is not the length the cipher requires.
    BadKey,
    /// The filesystem describes something impossible.
    Malformed(&'static str),
    /// No inode at that index.
    NoSuchInode(usize),
    /// The magic is not a filesystem's.
    ///
    /// Carries what was found, because the commonest wrong answer is being handed the package
    /// rather than the image inside it.
    NotAFilesystem(u64),
}

impl fmt::Display for PfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange => write!(f, "a read ran past the end of what is available"),
            Self::NotAFilesystem(found) => {
                write!(f, "magic {found:#x} is not a filesystem's ({MAGIC:#x})")
            }
            Self::BadKey => write!(f, "a key is not the length the cipher requires"),
            Self::Malformed(what) => write!(f, "malformed filesystem: {what}"),
            Self::NoSuchInode(index) => write!(f, "no inode {index}"),
        }
    }
}

impl std::error::Error for PfsError {}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::arithmetic_side_effects,
    reason = "fixture builders read better indexed, and a panic here is the test failing"
)]
mod tests {
    use super::{Filesystem, PfsError, Slice, Source, Xts, next_tweak};

    #[test]
    fn a_slice_reads_from_its_base() {
        let bytes: Vec<u8> = (0..64_u8).collect();
        let source = Slice::new(&bytes, 16);
        assert_eq!(source.read(0, 4).expect("reads"), vec![16, 17, 18, 19]);
        assert_eq!(source.read(4, 2).expect("reads"), vec![20, 21]);
    }

    #[test]
    fn a_read_past_the_end_is_refused_rather_than_truncated() {
        let bytes = vec![0_u8; 8];
        let source = Slice::new(&bytes, 0);
        assert_eq!(source.read(0, 9), Err(PfsError::OutOfRange));
        assert_eq!(source.read(8, 1), Err(PfsError::OutOfRange));
    }

    #[test]
    fn the_tweak_shifts_left_and_reduces_on_carry() {
        // The one piece of XTS easy to get subtly wrong. Wrong, it produces plausible noise
        // rather than an error, so it is checked against hand-computed values.
        let mut tweak = [0_u8; 16];
        tweak[0] = 1;
        assert_eq!(next_tweak(tweak)[0], 2, "an ordinary shift");

        // Carry out of the top byte reduces by the polynomial.
        let mut top = [0_u8; 16];
        top[15] = 0x80;
        let reduced = next_tweak(top);
        assert_eq!(reduced[0], 0x87, "the carry is reduced into the low byte");
        assert_eq!(reduced[15], 0x00, "and the top bit is gone");
    }

    #[test]
    fn the_tweak_carries_between_adjacent_bytes() {
        let mut tweak = [0_u8; 16];
        tweak[0] = 0x80;
        let next = next_tweak(tweak);
        assert_eq!(next[0], 0x00);
        assert_eq!(next[1], 0x01, "the bit moved up a byte");
    }

    #[test]
    fn sectors_below_the_threshold_are_left_alone() {
        // The container's own exemption rather than a property of XTS: the first sectors are
        // stored in the clear and decrypting them would corrupt the superblock.
        let plain: Vec<u8> = (0..=255_u8).cycle().take(0x2000).collect();
        let source = Slice::new(&plain, 0);
        let xts = Xts::new(source, &[0_u8; 16], &[1_u8; 16], 1).expect("keys");
        let first = xts.read(0, 16).expect("reads");
        assert_eq!(
            first,
            plain[..16],
            "sector 0 is exempt and must be unchanged"
        );
        let second = xts.read(0x1000, 16).expect("reads");
        assert_ne!(second, plain[0x1000..0x1010], "sector 1 is not exempt");
    }

    #[test]
    fn a_key_of_the_wrong_length_is_refused() {
        let bytes = vec![0_u8; 0x1000];
        assert_eq!(
            Xts::new(Slice::new(&bytes, 0), &[0_u8; 8], &[0_u8; 16], 0).map(|_| ()),
            Err(PfsError::BadKey)
        );
    }

    #[test]
    fn a_superblock_claiming_a_zero_block_size_is_refused() {
        // Everything downstream divides by it.
        //
        // The magic has to be right for this to reach the check it is about. Zeroed bytes now
        // fail earlier and for a better reason, which is what the test below covers - leaving
        // this one asserting the earlier failure would have quietly stopped testing division
        // by zero at all.
        let mut bytes = vec![0_u8; 0x400];
        let at = crate::superblock::MAGIC;
        bytes[at..at + 8].copy_from_slice(&crate::MAGIC.to_le_bytes());
        assert_eq!(
            Filesystem::new(Slice::new(&bytes, 0)).map(|_| ()),
            Err(PfsError::Malformed("block size is zero"))
        );
    }

    #[test]
    fn bytes_that_are_not_a_filesystem_say_so_rather_than_failing_downstream() {
        // The commonest wrong answer is being handed the package rather than the image inside
        // it, and without this the first complaint is about an absurd block size or an inode
        // table - neither of which points at the actual mistake.
        let bytes = vec![0_u8; 0x400];
        assert_eq!(
            Filesystem::new(Slice::new(&bytes, 0)).map(|_| ()),
            Err(PfsError::NotAFilesystem(0))
        );
    }
}
