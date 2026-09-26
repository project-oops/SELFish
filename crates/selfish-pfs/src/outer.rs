//! The outer filesystem of a package: signed, encrypted, and holding one file.
//!
//! The inner filesystem, the files a title is made of, is plain and [`crate::write`] builds it.
//! The outer one holds exactly one file, `pfs_image.dat`, the inner image inside a
//! [`crate::pfsc`] container, and carries the signatures and encryption. Every key comes from
//! `EKPFS`, which comes from the content id and the passcode:
//!
//! ```text
//! sign key  = HMAC-SHA256(EKPFS, LE32(2) || seed)
//! xts keys  = HMAC-SHA256(EKPFS, LE32(1) || seed)   -> tweak = [0..16], data = [16..32]
//! ```
//!
//! A signature is an `HMAC-SHA256` of a block under the sign key: a keyed digest, not a claim
//! of authorship.
//!
//! ```text
//! block 0            the header
//! blocks 1..=N       the inode table, signed inodes of 0x2C8
//! block N+1          the super root's directory entries
//! block N+2          the flat path table
//! block N+3          an empty block, left unencrypted
//! blocks N+4..       indirect signature blocks
//! then               the root's entries, then the file
//! ```

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::write::{dirent, imode, kind};
use crate::{PfsError, mode, put, put_le, superblock};

/// The name the outer filesystem gives its single file.
pub const IMAGE_NAME: &str = "pfs_image.dat";

/// Size of a signed inode.
const INODE_SIZE: usize = 0x2C8;
/// Where an inode's block signatures begin: `0x64 + 36 * n` in `LibOrbisPkg@6434772`, and the
/// reader here finds the block number at `0x84`, past the 32-byte digest.
const INODE_SIG_AT: usize = 0x64;
/// How much one block signature occupies: a digest and the block it covers.
const SIG_SIZE: usize = 36;
/// How many direct block signatures an inode holds before it needs an indirect block.
const DIRECT: usize = 12;
/// Where the header's own signature goes.
const HEADER_SIG_AT: usize = 0x380;
/// How much of the header that signature covers.
const HEADER_SIG_LEN: usize = 0x5A0;
/// Where the embedded inode's block signatures begin.
const HEADER_INODE_SIG_AT: usize = 0xB8;
/// Where a `PFSC` header records the length its contents decompress to.
const PFSC_DATA_LENGTH: usize = 0x28;
/// One XTS sector.
const SECTOR: usize = 0x1000;
/// How many sectors at the front are left in the clear: the whole header block.
const PLAIN_SECTORS: u64 = 16;

/// The four inodes an outer filesystem always has.
mod ino {
    /// The super root, which names the path table and the root.
    pub(super) const SUPER_ROOT: u32 = 0;
    /// The flat path table.
    pub(super) const FPT: u32 = 1;
    /// The root directory.
    pub(super) const UROOT: u32 = 2;
    /// The single file, which is the inner image.
    pub(super) const FILE: u32 = 3;
}
/// How many of them there are.
const INODE_COUNT: usize = 4;

/// Inode flags.
mod iflag {
    /// The flags every inode of a real package's outer filesystem carries. With any other
    /// value the hardware fails to open `pfs_image.dat` (`ENOENT`, `mountApp0Dir 0x80020002`).
    /// Reproduced, not interpreted.
    pub(super) const READ_ONLY: u32 = 0x0C;
    /// The compressed bit. A real package sets it on `pfs_image.dat` and nothing else.
    pub(super) const COMPRESSED: u32 = 0x1;
    /// Internal to the filesystem, which is what the super root and path table are.
    pub(super) const INTERNAL: u32 = 0x2_0000;
}

/// Offsets within an inode that this module writes.
mod field {
    /// Link count.
    pub(super) const NLINK: usize = 0x02;
    /// Flags.
    pub(super) const FLAGS: usize = 0x04;
    /// Size.
    pub(super) const SIZE: usize = 0x08;
    /// Size again, uncompressed.
    pub(super) const SIZE_COMPRESSED: usize = 0x10;
    /// Block count.
    pub(super) const BLOCKS: usize = 0x60;
}

/// Derive the key that signs blocks.
///
/// # Errors
///
/// If the key is not usable as an HMAC key.
pub fn sign_key(ekpfs: &[u8], seed: &[u8]) -> Result<[u8; 32], PfsError> {
    derive(ekpfs, seed, 2)
}

/// Derive the XTS tweak and data keys.
///
/// The same derivation as [`crate::image_keys`], reached from the key rather than from a
/// superblock, because a builder has the seed before it has a superblock.
///
/// # Errors
///
/// If the key is not usable as an HMAC key.
pub fn encryption_keys(ekpfs: &[u8], seed: &[u8]) -> Result<([u8; 16], [u8; 16]), PfsError> {
    let derived = derive(ekpfs, seed, 1)?;
    let mut tweak = [0_u8; 16];
    let mut data = [0_u8; 16];
    tweak.copy_from_slice(derived.get(..16).ok_or(PfsError::BadKey)?);
    data.copy_from_slice(derived.get(16..32).ok_or(PfsError::BadKey)?);
    Ok((tweak, data))
}

fn derive(ekpfs: &[u8], seed: &[u8], index: u32) -> Result<[u8; 32], PfsError> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(ekpfs).map_err(|_| PfsError::BadKey)?;
    mac.update(&index.to_le_bytes());
    mac.update(seed);
    let out = mac.finalize().into_bytes();
    let mut key = [0_u8; 32];
    key.copy_from_slice(out.get(..32).ok_or(PfsError::BadKey)?);
    Ok(key)
}

/// What to build.
#[derive(Debug, Clone)]
pub struct Options<'a> {
    /// The file's contents. In a package this is a `PFSC` container holding the inner image.
    pub payload: &'a [u8],
    /// The filesystem key, from the content id and the passcode.
    pub ekpfs: &'a [u8],
    /// The seed the keys are derived against. Zero in every image examined.
    pub seed: [u8; 16],
    /// Whether to encrypt. A package encrypts; leaving it off is useful for inspecting one.
    pub encrypt: bool,
    /// Block size. Every real image uses `0x10000`.
    pub block_size: u32,
}

/// One block signature waiting to be computed: which block it covers, how much of it, and
/// where the result goes.
struct Pending {
    block: u32,
    at: usize,
    len: usize,
}

/// Where each part of the image goes, and the signatures each part owes.
struct Layout {
    block: usize,
    inode_blocks: usize,
    super_root: u32,
    fpt: u32,
    /// The one block after the header that is not encrypted.
    empty: u32,
    root_start: u32,
    root_blocks: usize,
    file_start: u32,
    payload_blocks: usize,
    total: u32,
    /// Signatures over data blocks, computed first.
    sigs: Vec<Pending>,
    /// Signatures over blocks that hold other signatures, computed after them in reverse.
    deferred: Vec<Pending>,
}

/// The super root's entries, the root's entries, and the path table.
struct Directories {
    super_root: Vec<u8>,
    root: Vec<u8>,
    fpt: Vec<u8>,
}

/// Build the outer image.
///
/// # Errors
///
/// If the block size cannot hold an inode, if the payload needs more indirection than a single
/// indirect block provides, or if the result does not fit the format's 32-bit block numbers.
pub fn build(options: &Options<'_>) -> Result<Vec<u8>, PfsError> {
    let block = usize::try_from(options.block_size).map_err(|_| PfsError::OutOfRange)?;
    if block == 0 {
        return Err(PfsError::Malformed("block size is zero"));
    }
    let dirs = directories();
    let layout = plan(block, options.payload.len(), dirs.root.len())?;

    let total = usize::try_from(layout.total).map_err(|_| PfsError::OutOfRange)?;
    let mut out = vec![0_u8; total.checked_mul(block).ok_or(PfsError::OutOfRange)?];
    write_header(&mut out, options, layout.inode_blocks, layout.total)?;
    write_inodes(&mut out, options.payload, &layout, dirs.fpt.len())?;
    place(&mut out, layout.super_root, block, &dirs.super_root)?;
    place(&mut out, layout.fpt, block, &dirs.fpt)?;
    place(&mut out, layout.root_start, block, &dirs.root)?;
    place(&mut out, layout.file_start, block, options.payload)?;

    sign(&mut out, &layout, &sign_key(options.ekpfs, &options.seed)?)?;
    if options.encrypt {
        let (tweak, data) = encryption_keys(options.ekpfs, &options.seed)?;
        encrypt(&mut out, &tweak, &data, block, layout.empty)?;
    }
    Ok(out)
}

/// The directory contents, which are fixed before any block is placed.
fn directories() -> Directories {
    let mut super_root = Vec::new();
    dirent(
        &mut super_root,
        ino::FPT,
        kind::FILE,
        crate::write::FLAT_PATH_TABLE,
    );
    dirent(
        &mut super_root,
        ino::UROOT,
        kind::DIRECTORY,
        crate::write::ROOT_NAME,
    );

    // The root's parent is itself, as in a real image, not the super root.
    let mut root = Vec::new();
    dirent(&mut root, ino::UROOT, kind::DOT, ".");
    dirent(&mut root, ino::UROOT, kind::DOT_DOT, "..");
    dirent(&mut root, ino::FILE, kind::FILE, IMAGE_NAME);

    // One path table entry, for the one file; the hardware finds `pfs_image.dat` through it.
    let fpt = crate::write::path_table_entry(&format!("/{IMAGE_NAME}"), ino::FILE, false);
    Directories {
        super_root,
        root,
        fpt,
    }
}

/// Place every block, in the order of the module layout, and record the signature each owes.
///
/// Inode blocks, the super root and the path table are signed into the header's embedded inode
/// or their own inode. The root and file blocks are signed into their inodes, and file blocks
/// past the twelfth into the indirect block, which is itself signed into the file inode's
/// thirteenth slot once full.
fn plan(block: usize, payload_len: usize, root_len: usize) -> Result<Layout, PfsError> {
    let per_block = block
        .checked_div(INODE_SIZE)
        .filter(|n| *n > 0)
        .ok_or(PfsError::Malformed("a block holds no inodes"))?;
    let sigs_per_block = block
        .checked_div(SIG_SIZE)
        .filter(|n| *n > 0)
        .ok_or(PfsError::Malformed("a block holds no signatures"))?;
    let inode_blocks = INODE_COUNT.div_ceil(per_block);
    let payload_blocks = payload_len.div_ceil(block).max(1);
    let root_blocks = root_len.div_ceil(block).max(1);
    if payload_blocks
        > DIRECT
            .checked_add(sigs_per_block)
            .ok_or(PfsError::OutOfRange)?
    {
        return Err(PfsError::Malformed(
            "payload needs a doubly-indirect signature block, which is not built yet",
        ));
    }

    let mut blocks = Blocks {
        next: 0,
        size: block,
    };
    let mut deferred = Vec::new();
    blocks.take()?;
    for index in 0..inode_blocks {
        let at = index
            .checked_mul(SIG_SIZE)
            .and_then(|by| HEADER_INODE_SIG_AT.checked_add(by))
            .ok_or(PfsError::OutOfRange)?;
        deferred.push(blocks.signed(at)?);
    }
    let super_root = blocks.next;
    deferred.push(blocks.signed(inode_sig_at(block, ino::SUPER_ROOT, 0)?)?);
    let fpt = blocks.next;
    deferred.push(blocks.signed(inode_sig_at(block, ino::FPT, 0)?)?);
    // Reproduced from `LibOrbisPkg@6434772`, which records it as unexplained.
    let empty = blocks.take()?;

    // The indirect block is placed before the file blocks whose signatures it holds.
    let indirect = (payload_blocks > DIRECT).then_some(blocks.next);
    if indirect.is_some() {
        blocks.take()?;
    }

    let mut sigs = Vec::new();
    let root_start = blocks.next;
    for index in 0..root_blocks {
        sigs.push(blocks.signed(inode_sig_at(block, ino::UROOT, index)?)?);
    }
    let file_start = blocks.next;
    for index in 0..payload_blocks {
        let at = match indirect {
            Some(indirect) if index >= DIRECT => usize::try_from(indirect)
                .map_err(|_| PfsError::OutOfRange)?
                .checked_mul(block)
                .and_then(|base| {
                    let slot = index.checked_sub(DIRECT)?.checked_mul(SIG_SIZE)?;
                    base.checked_add(slot)
                })
                .ok_or(PfsError::OutOfRange)?,
            _ => inode_sig_at(block, ino::FILE, index)?,
        };
        sigs.push(blocks.signed(at)?);
    }
    if let Some(indirect) = indirect {
        deferred.push(Pending {
            block: indirect,
            at: inode_sig_at(block, ino::FILE, DIRECT)?,
            len: block,
        });
    }

    Ok(Layout {
        block,
        inode_blocks,
        super_root,
        fpt,
        empty,
        root_start,
        root_blocks,
        file_start,
        payload_blocks,
        total: blocks.next,
        sigs,
        deferred,
    })
}

/// The next free block, and the block size.
struct Blocks {
    next: u32,
    size: usize,
}

impl Blocks {
    /// Take the next block.
    fn take(&mut self) -> Result<u32, PfsError> {
        let taken = self.next;
        self.next = self.next.checked_add(1).ok_or(PfsError::OutOfRange)?;
        Ok(taken)
    }

    /// Take the next block, with its whole-block signature written at `at`.
    fn signed(&mut self, at: usize) -> Result<Pending, PfsError> {
        Ok(Pending {
            block: self.take()?,
            at,
            len: self.size,
        })
    }
}

/// Write the four inodes into the table in block one.
///
/// `pfs_image.dat` carries the compressed bit, which tells the hardware to decompress the
/// `PFSC` container before mounting the inner image; without it `nmount()` refuses the raw
/// container with `EINVAL`. Its `size` is the container's length and its `size_compressed` the
/// length the container decompresses to, read from the `PFSC` header.
fn write_inodes(
    out: &mut [u8],
    payload: &[u8],
    layout: &Layout,
    fpt_len: usize,
) -> Result<(), PfsError> {
    let block = layout.block;
    let root_size = layout
        .root_blocks
        .checked_mul(block)
        .ok_or(PfsError::OutOfRange)?;
    let inodes = [
        (
            ino::SUPER_ROOT,
            imode::DIR | imode::RX,
            iflag::INTERNAL | iflag::READ_ONLY,
            block,
            1,
            layout.super_root,
        ),
        (
            ino::FPT,
            imode::FILE | imode::RX,
            iflag::INTERNAL | iflag::READ_ONLY,
            fpt_len,
            1,
            layout.fpt,
        ),
        (
            ino::UROOT,
            imode::DIR | imode::RX,
            iflag::READ_ONLY,
            root_size,
            layout.root_blocks,
            layout.root_start,
        ),
        (
            ino::FILE,
            imode::FILE | imode::RX,
            iflag::READ_ONLY | iflag::COMPRESSED,
            payload.len(),
            layout.payload_blocks,
            layout.file_start,
        ),
    ];
    for (number, mode, flags, size, blocks, start) in inodes {
        let at = inode_at(block, number)?;
        write_inode(out, at, mode, flags, (size, blocks), start)?;
    }

    // A payload too short to hold a `PFSC` header is not one, and its length is its own.
    let inner_size = match selfish_bytes::read_le(payload, PFSC_DATA_LENGTH) {
        Some(value) => value,
        None => u64::try_from(payload.len()).map_err(|_| PfsError::OutOfRange)?,
    };
    let file_at = inode_at(block, ino::FILE)?;
    put_le(out, offset(file_at, field::SIZE_COMPRESSED)?, inner_size)
}

/// Compute every signature in dependency order.
///
/// Data blocks first, because an indirect block's content is their signatures. Then the
/// deferred ones in reverse, so the indirect block is signed once full and the inode blocks
/// once complete. The header is last: its digest covers where the inode signatures live.
fn sign(out: &mut [u8], layout: &Layout, key: &[u8; 32]) -> Result<(), PfsError> {
    let header = Pending {
        block: 0,
        at: HEADER_SIG_AT,
        len: HEADER_SIG_LEN,
    };
    let order = layout
        .sigs
        .iter()
        .chain(layout.deferred.iter().rev())
        .chain([&header]);
    for sig in order {
        sign_block(out, layout.block, key, sig)?;
    }
    Ok(())
}

/// Compute one block signature and write it, followed by the block it covers.
///
/// The digest is taken before it is written, which matters for the header: its signature sits
/// inside the range it covers, so a verifier zeroes the slot before recomputing.
fn sign_block(out: &mut [u8], block: usize, key: &[u8; 32], sig: &Pending) -> Result<(), PfsError> {
    let from = usize::try_from(sig.block)
        .map_err(|_| PfsError::OutOfRange)?
        .checked_mul(block)
        .ok_or(PfsError::OutOfRange)?;
    let to = from.checked_add(sig.len).ok_or(PfsError::OutOfRange)?;
    let body = out.get(from..to).ok_or(PfsError::OutOfRange)?;

    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).map_err(|_| PfsError::BadKey)?;
    mac.update(body);
    let digest = mac.finalize().into_bytes();

    put(out, sig.at, &digest)?;
    put_le(out, offset(sig.at, 32)?, sig.block)
}

/// Encrypt every sector except the header block and the empty block.
fn encrypt(
    out: &mut [u8],
    tweak_key: &[u8; 16],
    data_key: &[u8; 16],
    block: usize,
    empty_block: u32,
) -> Result<(), PfsError> {
    use aes::cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray};

    let tweak_cipher = aes::Aes128::new_from_slice(tweak_key).map_err(|_| PfsError::BadKey)?;
    let data_cipher = aes::Aes128::new_from_slice(data_key).map_err(|_| PfsError::BadKey)?;
    let sectors_per_block = block.div_ceil(SECTOR);
    let skip_from = usize::try_from(empty_block)
        .map_err(|_| PfsError::OutOfRange)?
        .checked_mul(sectors_per_block)
        .ok_or(PfsError::OutOfRange)?;
    let skip_to = skip_from
        .checked_add(sectors_per_block)
        .ok_or(PfsError::OutOfRange)?;

    let total = out.len().div_ceil(SECTOR);
    for index in usize::try_from(PLAIN_SECTORS).map_err(|_| PfsError::OutOfRange)?..total {
        if (skip_from..skip_to).contains(&index) {
            continue;
        }
        let at = index.checked_mul(SECTOR).ok_or(PfsError::OutOfRange)?;
        let end = at.checked_add(SECTOR).ok_or(PfsError::OutOfRange)?;
        let sector = out.get_mut(at..end).ok_or(PfsError::OutOfRange)?;

        // XTS: the sector number, little-endian in sixteen bytes, encrypted under the tweak
        // key, then advanced through the sector by doubling in GF(2^128).
        let mut tweak = [0_u8; 16];
        let number = u64::try_from(index).map_err(|_| PfsError::OutOfRange)?;
        put_le(&mut tweak, 0, number)?;
        tweak_cipher.encrypt_block(GenericArray::from_mut_slice(&mut tweak));

        for chunk in sector.chunks_mut(16) {
            for (byte, mask) in chunk.iter_mut().zip(tweak.iter()) {
                *byte ^= *mask;
            }
            data_cipher.encrypt_block(GenericArray::from_mut_slice(chunk));
            for (byte, mask) in chunk.iter_mut().zip(tweak.iter()) {
                *byte ^= *mask;
            }
            tweak = advance(tweak);
        }
    }
    Ok(())
}

/// Multiply the tweak by two in GF(2^128), which is how XTS steps from one block to the next.
fn advance(tweak: [u8; 16]) -> [u8; 16] {
    let mut out = [0_u8; 16];
    let mut carry = 0_u8;
    for (index, byte) in tweak.iter().enumerate() {
        if let Some(slot) = out.get_mut(index) {
            *slot = (byte << 1) | carry;
        }
        carry = byte >> 7;
    }
    if carry != 0
        && let Some(first) = out.first_mut()
    {
        *first ^= 0x87;
    }
    out
}

/// Write the header into block zero.
///
/// The header embeds an inode describing the inode table, whose block signatures are the ones
/// at [`HEADER_INODE_SIG_AT`]. Only the fields a real image sets are written.
fn write_header(
    out: &mut [u8],
    options: &Options<'_>,
    inode_blocks: usize,
    total_blocks: u32,
) -> Result<(), PfsError> {
    let too_large = |_| PfsError::OutOfRange;
    put_le(out, superblock::VERSION, 1_u64)?;
    put_le(out, superblock::MAGIC, crate::MAGIC)?;
    if let Some(byte) = out.get_mut(superblock::READ_ONLY) {
        *byte = 1;
    }
    let mut flags = mode::SIGNED | mode::UNKNOWN_ALWAYS_SET;
    if options.encrypt {
        flags |= mode::ENCRYPTED;
    }
    put_le(out, superblock::MODE, flags)?;
    put_le(out, superblock::BLOCK_SIZE, options.block_size)?;
    put_le(out, superblock::N_BLOCK, 1_u64)?;
    put_le(
        out,
        superblock::INODE_COUNT,
        u64::try_from(INODE_COUNT).map_err(too_large)?,
    )?;
    put_le(out, superblock::N_DBLOCK, u64::from(total_blocks))?;
    let inode_blocks_64 = u64::try_from(inode_blocks).map_err(too_large)?;
    put_le(out, superblock::INODE_BLOCKS, inode_blocks_64)?;

    let embedded = HEADER_INODE_SIG_AT
        .checked_sub(0x68)
        .ok_or(PfsError::OutOfRange)?;
    let table_len = inode_blocks_64
        .checked_mul(u64::from(options.block_size))
        .ok_or(PfsError::OutOfRange)?;
    put_le(out, offset(embedded, field::NLINK)?, 1_u16)?;
    put_le(out, offset(embedded, field::FLAGS)?, iflag::READ_ONLY)?;
    put_le(out, offset(embedded, field::SIZE)?, table_len)?;
    put_le(out, offset(embedded, field::SIZE_COMPRESSED)?, table_len)?;
    put_le(
        out,
        offset(embedded, field::BLOCKS)?,
        u32::try_from(inode_blocks).map_err(too_large)?,
    )?;

    // A seeded image writes its index here; an unseeded one writes four bytes earlier.
    put_le(out, superblock::UNKNOWN_INDEX, 1_u32)?;
    put(out, superblock::SEED, &options.seed)
}

/// Write one signed inode. `extent` is the size in bytes and the block count.
///
/// The first block number also goes in the first signature slot, past its digest, so an
/// unsigned image is still readable; signing writes the same value again.
fn write_inode(
    out: &mut [u8],
    at: usize,
    mode: u16,
    flags: u32,
    (size, blocks): (usize, usize),
    start: u32,
) -> Result<(), PfsError> {
    let size = u64::try_from(size).map_err(|_| PfsError::OutOfRange)?;
    put_le(out, at, mode)?;
    put_le(out, offset(at, field::NLINK)?, 1_u16)?;
    put_le(out, offset(at, field::FLAGS)?, flags)?;
    put_le(out, offset(at, field::SIZE)?, size)?;
    put_le(out, offset(at, field::SIZE_COMPRESSED)?, size)?;
    put_le(
        out,
        offset(at, field::BLOCKS)?,
        u32::try_from(blocks).map_err(|_| PfsError::OutOfRange)?,
    )?;
    put_le(out, offset(at, INODE_SIG_AT + 32)?, start)
}

fn offset(base: usize, field: usize) -> Result<usize, PfsError> {
    base.checked_add(field).ok_or(PfsError::OutOfRange)
}

/// Where one inode begins.
fn inode_at(table: usize, number: u32) -> Result<usize, PfsError> {
    usize::try_from(number)
        .map_err(|_| PfsError::OutOfRange)?
        .checked_mul(INODE_SIZE)
        .and_then(|offset| table.checked_add(offset))
        .ok_or(PfsError::OutOfRange)
}

/// Where one of an inode's block signatures goes.
fn inode_sig_at(block: usize, number: u32, index: usize) -> Result<usize, PfsError> {
    inode_at(block, number)?
        .checked_add(INODE_SIG_AT)
        .and_then(|at| index.checked_mul(SIG_SIZE).and_then(|o| at.checked_add(o)))
        .ok_or(PfsError::OutOfRange)
}

/// Copy a body into place at a block boundary.
fn place(out: &mut [u8], start: u32, block: usize, body: &[u8]) -> Result<(), PfsError> {
    let at = usize::try_from(start)
        .map_err(|_| PfsError::OutOfRange)?
        .checked_mul(block)
        .ok_or(PfsError::OutOfRange)?;
    put(out, at, body)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a panic in a test is the test failing, which is what a test is for"
)]
mod tests {
    use super::{IMAGE_NAME, Options, build, encryption_keys, sign_key};
    use crate::{Filesystem, Slice, Superblock, Xts, pfsc, write};

    const BLOCK: u32 = 0x10000;
    const EKPFS: &[u8] = b"a sixteen-byte..";

    fn options(payload: &[u8], encrypt: bool) -> Options<'_> {
        Options {
            payload,
            ekpfs: EKPFS,
            seed: [0; 16],
            encrypt,
            block_size: BLOCK,
        }
    }

    /// The outer image's superblock declares it signed and sizes it exactly.
    #[test]
    fn the_outer_image_declares_itself_signed() {
        let image = build(&options(b"payload", false)).expect("an image");
        let sb = Superblock::parse(&image).expect("a superblock");
        assert!(sb.is_signed(), "the outer filesystem is always signed");
        assert!(!sb.is_encrypted(), "this one was asked not to be");
        assert_eq!(sb.image_len(), u64::try_from(image.len()).unwrap());
    }

    /// An unencrypted outer image reads back, with the reader using the signed inode stride.
    #[test]
    fn an_unencrypted_outer_image_reads_back_through_the_reader() {
        let payload = vec![0x5A_u8; 100_000];
        let image = build(&options(&payload, false)).expect("an image");
        let fs = Filesystem::new(Slice::new(&image, 0)).expect("a filesystem");
        let found = fs.walk(2).expect("a walk");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, format!("/{IMAGE_NAME}"));
        assert_eq!(fs.contents(found[0].inode).expect("bytes"), payload);
    }

    /// The writer's XTS encryption and the reader's XTS decryption agree.
    #[test]
    fn an_encrypted_image_decrypts_to_the_same_thing() {
        let payload = vec![0x33_u8; 90_000];
        let plain = build(&options(&payload, false)).expect("a plain image");
        let secret = build(&options(&payload, true)).expect("an encrypted image");
        assert_ne!(plain, secret, "encryption must actually do something");

        let (tweak, data) = encryption_keys(EKPFS, &[0; 16]).expect("keys");
        let source = Xts::new(Slice::new(&secret, 0), &tweak, &data, super::PLAIN_SECTORS)
            .expect("a decryptor");
        let fs = Filesystem::new(source).expect("a filesystem");
        let found = fs.walk(2).expect("a walk");
        assert_eq!(found.len(), 1);
        assert_eq!(fs.contents(found[0].inode).expect("bytes"), payload);
    }

    /// Inner filesystem, `PFSC` container and encrypted outer filesystem round-trip together.
    #[test]
    fn the_whole_package_filesystem_stack_round_trips() {
        let inner = write::Tree::new(write::ROOT_NAME)
            .with_file("eboot.bin", vec![0xE1; 3000])
            .with_dir(write::Tree::new("sce_sys").with_file("param.sfo", b"sfo".to_vec()));
        let inner_image = write::build(&inner, BLOCK).expect("an inner image");
        let wrapped = pfsc::wrap(&inner_image, BLOCK).expect("a container");
        let outer = build(&options(&wrapped, true)).expect("an outer image");

        let (tweak, data) = encryption_keys(EKPFS, &[0; 16]).expect("keys");
        let outer_fs = Filesystem::new(
            Xts::new(Slice::new(&outer, 0), &tweak, &data, super::PLAIN_SECTORS)
                .expect("a decryptor"),
        )
        .expect("the outer filesystem");
        let image_entry = outer_fs
            .walk(2)
            .expect("a walk")
            .into_iter()
            .find(|f| f.path == format!("/{IMAGE_NAME}"))
            .expect("the image file");
        let recovered = outer_fs.contents(image_entry.inode).expect("the container");
        assert_eq!(recovered, wrapped, "the container survives the outer layer");

        let inner_fs = Filesystem::new(
            crate::Compressed::new(Slice::new(&recovered, 0)).expect("a decompressor"),
        )
        .expect("the inner filesystem");
        let mut paths: Vec<String> = inner_fs
            .walk(write::ROOT_INODE)
            .expect("a walk")
            .into_iter()
            .map(|f| f.path)
            .collect();
        paths.sort();
        assert_eq!(paths, ["/eboot.bin", "/sce_sys/param.sfo"]);
    }

    /// The sign key and the encryption keys come from different derivation indices.
    #[test]
    fn the_two_derived_keys_differ() {
        let signing = sign_key(EKPFS, &[0; 16]).expect("a sign key");
        let (tweak, data) = encryption_keys(EKPFS, &[0; 16]).expect("enc keys");
        assert_ne!(&signing[..16], &tweak[..]);
        assert_ne!(&signing[16..], &data[..]);
    }

    /// A payload needing a doubly-indirect signature block is refused.
    #[test]
    fn a_payload_needing_double_indirection_is_refused_rather_than_guessed() {
        // Just past twelve direct blocks plus one block of signatures.
        let too_big = vec![0_u8; (12 + 0x10000 / 36 + 1) * BLOCK as usize];
        assert!(build(&options(&too_big, false)).is_err());
    }
}
