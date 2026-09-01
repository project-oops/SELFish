//! The outer filesystem of a package: signed, encrypted, and holding one file.
//!
//! A package nests two filesystems, and they are built to different rules. The inner one - the
//! files a title is actually made of - is plain, and [`crate::write`] builds it. The outer one
//! holds exactly one file, `pfs_image.dat`, which is the inner image inside a [`crate::pfsc`]
//! container, and it is the layer that carries signatures and encryption.
//!
//! # No key is missing here
//!
//! Both the signing key and the encryption keys come from `EKPFS`, and `EKPFS` comes from the
//! content id and the passcode by the same derivation a package's entry keys use. A caller who
//! can name the content id and the passcode can build this image; nothing has to be recovered
//! from anywhere.
//!
//! ```text
//! sign key  = HMAC-SHA256(EKPFS, LE32(2) || seed)
//! xts keys  = HMAC-SHA256(EKPFS, LE32(1) || seed)   -> tweak = [0..16], data = [16..32]
//! ```
//!
//! # Signatures, and what they are not
//!
//! A signature here is an `HMAC-SHA256` of a block under a key both sides derive - a keyed
//! digest, not an RSA signature and not a claim of authorship. Every one of them is computed
//! from a passcode the builder chose. Nothing is forged and nothing could be.
//!
//! # Layout
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
use crate::{PfsError, mode, superblock};

/// The name the outer filesystem gives its single file.
pub const IMAGE_NAME: &str = "pfs_image.dat";

/// Size of a signed inode.
const INODE_SIZE: usize = 0x2C8;
/// Where an inode's block signatures begin.
///
/// `LibOrbisPkg@6434772` computes it as `0x64 + 36 * n`. The reader in this crate independently
/// measured the *block number* at `0x84`, which is this plus the 32 bytes of digest in front
/// of it. Two derivations, one layout.
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
///
/// Read out of the payload rather than recomputed: `pfs_image.dat` is a `PFSC` container, and the
/// inode's `size_compressed` holds the *inner* image's length while `size` holds the container's.
const PFSC_DATA_LENGTH: core::ops::Range<usize> = 0x28..0x30;
/// One XTS sector.
const SECTOR: usize = 0x1000;
/// How many sectors at the front are left in the clear: the whole header block.
const PLAIN_SECTORS: u64 = 16;

/// The four inodes an outer filesystem always has. There is no tree to walk.
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
    /// The flags every inode in a real package's outer filesystem carries, `0x0C`.
    ///
    /// This crate wrote `0x10` here, on the reasoning that it was a read-only bit. A console
    /// mounts the outer image and then opens `pfs_image.dat` from the root of that mount; with
    /// `0x10` the open fails with `ENOENT` and the launch dies at `mountApp0Dir 0x80020002`,
    /// while every inode in three real packages carries `0x0C` (measured: super root, path
    /// table, uroot and the file all agree, and the file adds only [`COMPRESSED`]). The bit's
    /// meaning is not interpreted - it is reproduced because the console requires it. (measured)
    pub(super) const READ_ONLY: u32 = 0x0C;
    /// The compressed bit, `0x1`. A real package sets it on `pfs_image.dat` and on nothing else.
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
/// superblock, because a builder has the seed before it has a superblock to read it out of.
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

/// Build the outer image.
///
/// # Errors
///
/// If the block size cannot hold an inode, if the payload needs more indirection than a single
/// indirect block provides, or if the result does not fit the format's 32-bit block numbers.
#[allow(
    clippy::too_many_lines,
    reason = "the layout is one sequence of decisions and splitting it hides the block order, \
              which is the only thing that makes it correct"
)]
pub fn build(options: &Options<'_>) -> Result<Vec<u8>, PfsError> {
    let block = usize::try_from(options.block_size).map_err(|_| PfsError::OutOfRange)?;
    if block == 0 {
        return Err(PfsError::Malformed("block size is zero"));
    }
    let per_block = block
        .checked_div(INODE_SIZE)
        .filter(|n| *n > 0)
        .ok_or(PfsError::Malformed("a block holds no inodes"))?;
    let sigs_per_block = block
        .checked_div(SIG_SIZE)
        .filter(|n| *n > 0)
        .ok_or(PfsError::Malformed("a block holds no signatures"))?;

    let inode_count = INODE_COUNT;
    let inode_blocks = inode_count.div_ceil(per_block);

    // ---- the two directories' contents, which are known before any block is placed --------
    let mut super_entries = Vec::new();
    dirent(
        &mut super_entries,
        ino::FPT,
        kind::FILE,
        crate::write::FLAT_PATH_TABLE,
    );
    dirent(
        &mut super_entries,
        ino::UROOT,
        kind::DIRECTORY,
        crate::write::ROOT_NAME,
    );

    let mut root_entries = Vec::new();
    dirent(&mut root_entries, ino::UROOT, kind::DOT, ".");
    // The root's parent is itself. Pointing it at the super root would be the obvious guess and
    // is not what a real image does.
    dirent(&mut root_entries, ino::UROOT, kind::DOT_DOT, "..");
    dirent(&mut root_entries, ino::FILE, kind::FILE, IMAGE_NAME);

    // The outer filesystem's path table has exactly one entry, for its one file. Built the
    // same way the inner one is, because a console reads this table to find `pfs_image.dat`
    // and nothing in this crate ever reads it back.
    let fpt_body = crate::write::path_table_entry(&format!("/{IMAGE_NAME}"), ino::FILE, false);

    // ---- layout ---------------------------------------------------------------------------
    let mut sigs: Vec<Pending> = Vec::new();
    let mut deferred: Vec<Pending> = Vec::new();

    // Block 0 is the header, and the inode table follows it. Each inode block is signed into
    // the inode embedded in the header.
    let mut next = u32::try_from(inode_blocks.checked_add(1).ok_or(PfsError::OutOfRange)?)
        .map_err(|_| PfsError::OutOfRange)?;
    for index in 0..inode_blocks {
        deferred.push(Pending {
            block: u32::try_from(index.checked_add(1).ok_or(PfsError::OutOfRange)?)
                .map_err(|_| PfsError::OutOfRange)?,
            at: HEADER_INODE_SIG_AT
                .checked_add(index.checked_mul(SIG_SIZE).ok_or(PfsError::OutOfRange)?)
                .ok_or(PfsError::OutOfRange)?,
            len: block,
        });
    }

    let super_root_block = next;
    deferred.push(Pending {
        block: super_root_block,
        at: inode_sig_at(block, ino::SUPER_ROOT, 0)?,
        len: block,
    });
    next = next.checked_add(1).ok_or(PfsError::OutOfRange)?;

    let fpt_block = next;
    deferred.push(Pending {
        block: fpt_block,
        at: inode_sig_at(block, ino::FPT, 0)?,
        len: block,
    });
    next = next.checked_add(1).ok_or(PfsError::OutOfRange)?;

    // An empty block sits after the path table, and it is the one block the encryption skips.
    // `LibOrbisPkg@6434772` records it as unexplained; it is reproduced rather than tidied away.
    let empty_block = next;
    next = next.checked_add(1).ok_or(PfsError::OutOfRange)?;

    // Indirect signature blocks are reserved before the data they describe, because the data
    // blocks' signatures are written *into* them.
    let payload_blocks = options.payload.len().div_ceil(block).max(1);
    let root_blocks = root_entries.len().div_ceil(block).max(1);
    if payload_blocks
        > DIRECT
            .checked_add(sigs_per_block)
            .ok_or(PfsError::OutOfRange)?
    {
        // Beyond one indirect block the format uses a doubly-indirect one. Refusing is the
        // honest answer: emitting a plausible layout that has never been checked is how a
        // wrong offset becomes somebody else's afternoon.
        return Err(PfsError::Malformed(
            "payload needs a doubly-indirect signature block, which is not built yet",
        ));
    }
    let indirect_block = next;
    let indirect_used = u32::from(payload_blocks > DIRECT);
    next = next
        .checked_add(indirect_used)
        .ok_or(PfsError::OutOfRange)?;

    // The root directory, then the file. Both sign every block they own.
    let root_start = next;
    for index in 0..root_blocks {
        sigs.push(Pending {
            block: next,
            at: inode_sig_at(block, ino::UROOT, index)?,
            len: block,
        });
        next = next.checked_add(1).ok_or(PfsError::OutOfRange)?;
    }

    let file_start = next;
    for index in 0..payload_blocks {
        let at = if index < DIRECT {
            inode_sig_at(block, ino::FILE, index)?
        } else {
            // Past twelve, a block's signature lives in the indirect block instead of the
            // inode, at the same 36-byte stride.
            usize::try_from(indirect_block)
                .map_err(|_| PfsError::OutOfRange)?
                .checked_mul(block)
                .and_then(|base| {
                    index
                        .checked_sub(DIRECT)
                        .and_then(|slot| slot.checked_mul(SIG_SIZE))
                        .and_then(|offset| base.checked_add(offset))
                })
                .ok_or(PfsError::OutOfRange)?
        };
        sigs.push(Pending {
            block: next,
            at,
            len: block,
        });
        next = next.checked_add(1).ok_or(PfsError::OutOfRange)?;
    }
    if indirect_used == 1 {
        // The indirect block is itself signed into the inode's thirteenth slot, and only once
        // every signature inside it has been written.
        deferred.push(Pending {
            block: indirect_block,
            at: inode_sig_at(block, ino::FILE, DIRECT)?,
            len: block,
        });
    }

    let total = usize::try_from(next).map_err(|_| PfsError::OutOfRange)?;
    let mut out = vec![0_u8; total.checked_mul(block).ok_or(PfsError::OutOfRange)?];

    // ---- contents ---------------------------------------------------------------------------
    write_header(&mut out, options, inode_count, inode_blocks, next)?;

    let table = block;
    write_inode(
        &mut out,
        inode_at(table, ino::SUPER_ROOT)?,
        imode::DIR | imode::RX,
        iflag::INTERNAL | iflag::READ_ONLY,
        block,
        1,
        super_root_block,
    )?;
    write_inode(
        &mut out,
        inode_at(table, ino::FPT)?,
        imode::FILE | imode::RX,
        iflag::INTERNAL | iflag::READ_ONLY,
        fpt_body.len(),
        1,
        fpt_block,
    )?;
    write_inode(
        &mut out,
        inode_at(table, ino::UROOT)?,
        imode::DIR | imode::RX,
        iflag::READ_ONLY,
        root_blocks.checked_mul(block).ok_or(PfsError::OutOfRange)?,
        root_blocks,
        root_start,
    )?;
    // pfs_image.dat carries the compressed bit, and a console reads it to decide the file is a
    // `PFSC` container to decompress before mounting the inner image. Without it the raw `PFSC`
    // bytes are handed to `nmount()`, which refuses them `EINVAL` - the failure that stood after
    // the outer image itself mounted. The file's own byte length goes in `size`; the length it
    // decompresses to - the inner image's - goes in `size_compressed`, and the `PFSC` header
    // carries it at `0x28`. (measured against three packages)
    let file_at = inode_at(table, ino::FILE)?;
    write_inode(
        &mut out,
        file_at,
        imode::FILE | imode::RX,
        iflag::READ_ONLY | iflag::COMPRESSED,
        options.payload.len(),
        payload_blocks,
        file_start,
    )?;
    let inner_size = match options.payload.get(PFSC_DATA_LENGTH) {
        Some(bytes) => {
            let mut value = [0_u8; 8];
            value.copy_from_slice(bytes);
            u64::from_le_bytes(value)
        }
        // A payload too short to hold a `PFSC` header is not one, and the honest length is its
        // own. Nothing is invented: the field is copied where it exists and not guessed where
        // it does not.
        None => u64::try_from(options.payload.len()).map_err(|_| PfsError::OutOfRange)?,
    };
    put(
        &mut out,
        file_at
            .checked_add(field::SIZE_COMPRESSED)
            .ok_or(PfsError::OutOfRange)?,
        &inner_size.to_le_bytes(),
    )?;

    place(&mut out, super_root_block, block, &super_entries)?;
    place(&mut out, fpt_block, block, &fpt_body)?;
    place(&mut out, root_start, block, &root_entries)?;
    place(&mut out, file_start, block, options.payload)?;

    // ---- signing ----------------------------------------------------------------------------
    // Order is the whole correctness argument. Data blocks first, because an indirect block's
    // content is those signatures. Then the deferred ones in reverse, so the indirect block is
    // signed after it has been filled, the inode blocks after the inodes are complete, and the
    // header last of all - its digest covers the region the inode signatures live in.
    let key = sign_key(options.ekpfs, &options.seed)?;
    for sig in &sigs {
        sign_block(&mut out, block, &key, sig)?;
    }
    for sig in deferred.iter().rev() {
        sign_block(&mut out, block, &key, sig)?;
    }
    sign_block(
        &mut out,
        block,
        &key,
        &Pending {
            block: 0,
            at: HEADER_SIG_AT,
            len: HEADER_SIG_LEN,
        },
    )?;

    // ---- encryption -------------------------------------------------------------------------
    if options.encrypt {
        let (tweak, data) = encryption_keys(options.ekpfs, &options.seed)?;
        encrypt(&mut out, &tweak, &data, block, empty_block)?;
    }
    Ok(out)
}

/// Compute one block signature and write it, followed by the block it covers.
///
/// The digest is taken over the region *before* the digest is written into it, which matters
/// for the header: its signature sits inside the range it covers. A verifier has to zero the
/// slot before recomputing, which is what the source this was taken from does.
fn sign_block(out: &mut [u8], block: usize, key: &[u8; 32], sig: &Pending) -> Result<(), PfsError> {
    let from = usize::try_from(sig.block)
        .map_err(|_| PfsError::OutOfRange)?
        .checked_mul(block)
        .ok_or(PfsError::OutOfRange)?;
    let to = from.checked_add(sig.len).ok_or(PfsError::OutOfRange)?;
    let body = out.get(from..to).ok_or(PfsError::OutOfRange)?.to_vec();

    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).map_err(|_| PfsError::BadKey)?;
    mac.update(&body);
    let digest = mac.finalize().into_bytes();

    let end = sig.at.checked_add(32).ok_or(PfsError::OutOfRange)?;
    out.get_mut(sig.at..end)
        .ok_or(PfsError::OutOfRange)?
        .copy_from_slice(&digest);
    put(out, end, &sig.block.to_le_bytes())
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
        // key, then advanced through the sector by the usual doubling in GF(2^128).
        let mut tweak = [0_u8; 16];
        let number = u64::try_from(index).map_err(|_| PfsError::OutOfRange)?;
        tweak
            .get_mut(..8)
            .ok_or(PfsError::OutOfRange)?
            .copy_from_slice(&number.to_le_bytes());
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
fn write_header(
    out: &mut [u8],
    options: &Options<'_>,
    inode_count: usize,
    inode_blocks: usize,
    total_blocks: u32,
) -> Result<(), PfsError> {
    put(out, superblock::VERSION, &1_u64.to_le_bytes())?;
    put(out, superblock::MAGIC, &crate::MAGIC.to_le_bytes())?;
    if let Some(byte) = out.get_mut(superblock::READ_ONLY) {
        *byte = 1;
    }
    let mut flags = mode::SIGNED | mode::UNKNOWN_ALWAYS_SET;
    if options.encrypt {
        flags |= mode::ENCRYPTED;
    }
    put(out, superblock::MODE, &flags.to_le_bytes())?;
    put(
        out,
        superblock::BLOCK_SIZE,
        &options.block_size.to_le_bytes(),
    )?;
    put(out, superblock::N_BLOCK, &1_u64.to_le_bytes())?;
    put(
        out,
        superblock::INODE_COUNT,
        &u64::try_from(inode_count)
            .map_err(|_| PfsError::OutOfRange)?
            .to_le_bytes(),
    )?;
    put(
        out,
        superblock::N_DBLOCK,
        &u64::from(total_blocks).to_le_bytes(),
    )?;
    put(
        out,
        superblock::INODE_BLOCKS,
        &u64::try_from(inode_blocks)
            .map_err(|_| PfsError::OutOfRange)?
            .to_le_bytes(),
    )?;

    // The header embeds an inode describing the inode table itself, and its block signatures
    // are what `INODE_BLOCK_SIG` points at. Only the fields that are set in a real image.
    let embedded = HEADER_INODE_SIG_AT
        .checked_sub(0x68)
        .ok_or(PfsError::OutOfRange)?;
    put(
        out,
        embedded
            .checked_add(field::NLINK)
            .ok_or(PfsError::OutOfRange)?,
        &1_u16.to_le_bytes(),
    )?;
    put(
        out,
        embedded
            .checked_add(field::FLAGS)
            .ok_or(PfsError::OutOfRange)?,
        &iflag::READ_ONLY.to_le_bytes(),
    )?;
    let table_len = u64::try_from(inode_blocks)
        .map_err(|_| PfsError::OutOfRange)?
        .checked_mul(u64::from(options.block_size))
        .ok_or(PfsError::OutOfRange)?;
    put(
        out,
        embedded
            .checked_add(field::SIZE)
            .ok_or(PfsError::OutOfRange)?,
        &table_len.to_le_bytes(),
    )?;
    put(
        out,
        embedded
            .checked_add(field::SIZE_COMPRESSED)
            .ok_or(PfsError::OutOfRange)?,
        &table_len.to_le_bytes(),
    )?;
    put(
        out,
        embedded
            .checked_add(field::BLOCKS)
            .ok_or(PfsError::OutOfRange)?,
        &u32::try_from(inode_blocks)
            .map_err(|_| PfsError::OutOfRange)?
            .to_le_bytes(),
    )?;

    // A seeded image writes its index here; an unseeded one writes four bytes earlier.
    put(out, superblock::UNKNOWN_INDEX, &1_u32.to_le_bytes())?;
    put(out, superblock::SEED, &options.seed)
}

/// Write one signed inode.
fn write_inode(
    out: &mut [u8],
    at: usize,
    mode: u16,
    flags: u32,
    size: usize,
    blocks: usize,
    start: u32,
) -> Result<(), PfsError> {
    let size = u64::try_from(size).map_err(|_| PfsError::OutOfRange)?;
    put(out, at, &mode.to_le_bytes())?;
    put(
        out,
        at.checked_add(field::NLINK).ok_or(PfsError::OutOfRange)?,
        &1_u16.to_le_bytes(),
    )?;
    put(
        out,
        at.checked_add(field::FLAGS).ok_or(PfsError::OutOfRange)?,
        &flags.to_le_bytes(),
    )?;
    put(
        out,
        at.checked_add(field::SIZE).ok_or(PfsError::OutOfRange)?,
        &size.to_le_bytes(),
    )?;
    put(
        out,
        at.checked_add(field::SIZE_COMPRESSED)
            .ok_or(PfsError::OutOfRange)?,
        &size.to_le_bytes(),
    )?;
    put(
        out,
        at.checked_add(field::BLOCKS).ok_or(PfsError::OutOfRange)?,
        &u32::try_from(blocks)
            .map_err(|_| PfsError::OutOfRange)?
            .to_le_bytes(),
    )?;
    // The first block number goes in the first signature slot, past its digest. The signing
    // pass overwrites it with the same value; writing it here means an unsigned image is still
    // readable.
    put(
        out,
        at.checked_add(INODE_SIG_AT)
            .and_then(|slot| slot.checked_add(32))
            .ok_or(PfsError::OutOfRange)?,
        &start.to_le_bytes(),
    )
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
    let end = at.checked_add(body.len()).ok_or(PfsError::OutOfRange)?;
    out.get_mut(at..end)
        .ok_or(PfsError::OutOfRange)?
        .copy_from_slice(body);
    Ok(())
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

    #[test]
    fn the_outer_image_declares_itself_signed() {
        let image = build(&options(b"payload", false)).expect("an image");
        let sb = Superblock::parse(&image).expect("a superblock");
        assert!(sb.is_signed(), "the outer filesystem is always signed");
        assert!(!sb.is_encrypted(), "this one was asked not to be");
        assert_eq!(sb.image_len(), u64::try_from(image.len()).unwrap());
    }

    #[test]
    fn an_unencrypted_outer_image_reads_back_through_the_reader() {
        // Signed inodes are 0x2C8 rather than 0xA8, so this also proves the reader picks the
        // right stride from the mode flags - a wrong one puts every field somewhere else.
        let payload = vec![0x5A_u8; 100_000];
        let image = build(&options(&payload, false)).expect("an image");
        let fs = Filesystem::new(Slice::new(&image, 0)).expect("a filesystem");
        let found = fs.walk(2).expect("a walk");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, format!("/{IMAGE_NAME}"));
        assert_eq!(fs.contents(found[0].inode).expect("bytes"), payload);
    }

    #[test]
    fn an_encrypted_image_decrypts_to_the_same_thing() {
        // The encryption written here and the decryption already in this crate are separate
        // implementations of XTS. If either has the tweak wrong, this fails.
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

    #[test]
    fn the_whole_package_filesystem_stack_round_trips() {
        // The real shape: an inner plain filesystem, wrapped in PFSC, carried as the single
        // file of a signed and encrypted outer filesystem. Every layer this crate can write,
        // read back by every layer it can read.
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

    #[test]
    fn the_two_derived_keys_differ() {
        // One derivation with two indices. Reusing the wrong one would sign with the encryption
        // key and still produce an image that reads back fine here.
        let signing = sign_key(EKPFS, &[0; 16]).expect("a sign key");
        let (tweak, data) = encryption_keys(EKPFS, &[0; 16]).expect("enc keys");
        assert_ne!(&signing[..16], &tweak[..]);
        assert_ne!(&signing[16..], &data[..]);
    }

    #[test]
    fn a_payload_needing_double_indirection_is_refused_rather_than_guessed() {
        // Just past twelve direct blocks plus one block of signatures.
        let too_big = vec![0_u8; (12 + 0x10000 / 36 + 1) * BLOCK as usize];
        assert!(build(&options(&too_big, false)).is_err());
    }
}
