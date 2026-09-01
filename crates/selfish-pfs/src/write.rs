//! Building a filesystem image from a directory of files.
//!
//! The writing half of this crate. Reading was always the easy half - a reader follows four
//! numbers per inode and never looks at the rest - and it is exactly that asymmetry which left
//! the superblock's other fields unnamed and looking like a wall for as long as they did.
//!
//! # Plain images only, and that is a scope rather than a gap
//!
//! An image can be **signed** (every block carries a digest, and inodes grow from `0xA8` to
//! `0x2C8` to hold them) and **encrypted** (AES-XTS over every sector). This builds neither.
//! What it builds is the structure underneath both: a superblock, an inode table, directory
//! entries and a block allocation.
//!
//! That is the half that can be proved without a key. The tests build a tree, read it back
//! through this crate's own [`crate::Filesystem`], walk it and compare every file's bytes -
//! which exercises every offset, every count and every block number in the image. Signing and
//! encryption sit on top of a correct image and cannot rescue an incorrect one.
//!
//! # Layout
//!
//! ```text
//! block 0            the superblock
//! blocks 1..=N       the inode table
//! block N+1          the super root's directory entries
//! block N+2          the flat path table
//! block N+3          an empty block, which every real image leaves here
//! blocks N+4..       each node's data, in inode order
//! ```

use crate::{PfsError, inode, mode, superblock};

/// A directory being built.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    /// Its name. The root's is ignored - the format calls the root `uroot`.
    pub name: String,
    /// Subdirectories.
    pub dirs: Vec<Tree>,
    /// Files, as name and contents.
    pub files: Vec<(String, Vec<u8>)>,
}

impl Tree {
    /// An empty directory with a name.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ..Self::default()
        }
    }

    /// Add a file.
    #[must_use]
    pub fn with_file(mut self, name: &str, bytes: Vec<u8>) -> Self {
        self.files.push((name.to_owned(), bytes));
        self
    }

    /// Add a subdirectory.
    #[must_use]
    pub fn with_dir(mut self, dir: Self) -> Self {
        self.dirs.push(dir);
        self
    }
}

/// The name the format gives the root directory.
pub const ROOT_NAME: &str = "uroot";
/// The name of the table listing every path.
pub const FLAT_PATH_TABLE: &str = "flat_path_table";
/// The inode the root directory always gets.
///
/// Fixed because the super root is `0` and the flat path table is `1`, and both are created
/// before anything a caller supplied.
pub const ROOT_INODE: usize = 2;

/// A directory entry's fixed part, before the name.
const DIRENT_HEADER: usize = 16;
/// Entries are padded so the next one starts aligned.
const DIRENT_ALIGN: usize = 8;

/// What a directory entry points at.
///
/// Shared with [`crate::outer`], which builds a filesystem of its own. One table, because two
/// spellings of one format rule is how they drift apart. (D063)
pub(crate) mod kind {
    /// A file.
    pub(crate) const FILE: u32 = 2;
    /// A directory.
    pub(crate) const DIRECTORY: u32 = 3;
    /// The entry naming the directory itself.
    pub(crate) const DOT: u32 = 4;
    /// The entry naming its parent.
    pub(crate) const DOT_DOT: u32 = 5;
}

/// Inode mode bits. Shared with [`crate::outer`].
pub(crate) mod imode {
    /// A directory.
    pub(crate) const DIR: u16 = 16384;
    /// A file.
    pub(crate) const FILE: u16 = 32768;
    /// Read and execute for everyone, which is what a read-only image carries.
    pub(crate) const RX: u16 = 0o555;
}

/// Offsets within one inode that this module writes and the reader does not.
mod field {
    /// Link count.
    pub(super) const NLINK: usize = 0x02;
    /// Inode flags.
    pub(super) const FLAGS: usize = 0x04;
    /// Size again, compressed. Equal to the size, since nothing here compresses.
    pub(super) const SIZE_COMPRESSED: usize = 0x10;
}

/// Where the superblock embeds an inode describing the inode table itself.
///
/// A console's mount reads it to find the inodes, and an image with it left blank is refused by
/// `nmount()` with `EINVAL` after the outer image has already mounted. (measured)
const EMBEDDED_INODE: usize = 0x50;

/// The flags a real package's *inner* filesystem writes on its inodes.
///
/// This crate wrote zero, and an inner image built that way is refused by the kernel's mount:
/// with the outer inodes corrected a console gets as far as `nmount()` on the inner image and it
/// fails `EINVAL`. Every inode in a real inner filesystem carries `0x10`, and the two internal
/// inodes - the super root and the flat path table - add `0x2_0000`. (measured, three packages)
///
/// This is a *different* value from the outer filesystem's `0x0C`: the two layers do not share a
/// convention, and reproducing each is a measurement, not a deduction.
mod iflag {
    /// The flag every inner inode carries.
    pub(super) const BASE: u32 = 0x10;
    /// Added on the super root and the path table, which are internal to the filesystem.
    pub(super) const INTERNAL: u32 = 0x2_0000;
}

/// One node as the layout sees it: a mode, and the bytes it holds.
struct Node {
    mode: u16,
    /// The contents. For a directory these are its entries, already serialised.
    body: Vec<u8>,
    /// Which block its data starts at, filled in during layout.
    start: u32,
}

/// Build an image.
///
/// `block_size` is what the superblock declares; every real image uses `0x10000`.
///
/// # Errors
///
/// If `block_size` is zero or too small to hold an inode, if the tree nests deeper than a
/// reader will follow, or if it is too large for the 32-bit block numbers the format uses.
pub fn build(root: &Tree, block_size: u32) -> Result<Vec<u8>, PfsError> {
    let block = usize::try_from(block_size).map_err(|_| PfsError::OutOfRange)?;
    if block == 0 {
        return Err(PfsError::Malformed("block size is zero"));
    }

    // Inode numbers are assigned before anything is laid out, because a directory entry names
    // a child by number and a directory cannot be serialised until its children have them.
    let mut nodes: Vec<Node> = Vec::new();
    let super_root = push(&mut nodes, imode::DIR | imode::RX);
    let fpt = push(&mut nodes, imode::FILE | imode::RX);
    let uroot = push(&mut nodes, imode::DIR | imode::RX);

    // Every path, gathered while walking, so the flat path table lists what the image actually
    // holds rather than what a caller said it would.
    // Two paths that hash alike need a second file to disambiguate them, which this does not
    // write. Refusing is the honest answer: a table with a lost entry is a file the console
    // cannot find, and nothing in this crate reads the table, so nothing here would notice.
    if has_collision(root) {
        return Err(PfsError::Malformed(
            "two paths hash alike and a collision resolver is not built yet",
        ));
    }
    let mut paths = Vec::new();
    // **The root's parent is itself, not the super root.**
    //
    // `outer.rs` already states this rule for the filesystem it builds - *pointing it at the
    // super root would be the obvious guess and is not what a real image does* - and this builder
    // did exactly that obvious thing, so the two filesystems in one package disagreed about a
    // structure they share. A real inner image has `uroot`'s `..` naming `uroot`; this named the
    // super root, which is an internal directory a title's tree is not supposed to reach.
    //
    // Measured on three packages, and the kind of fault that does not return an error: anything
    // walking up from `/app0` left the tree instead of staying at its top. (D072)
    let root_entries = serialise_dir(root, uroot, uroot, &mut nodes, "", &mut paths, 0)?;
    set_body(&mut nodes, uroot, root_entries);

    // The super root names the flat path table and the root directory, and nothing else.
    let mut super_entries = Vec::new();
    dirent(&mut super_entries, fpt, kind::FILE, FLAT_PATH_TABLE);
    dirent(&mut super_entries, uroot, kind::DIRECTORY, ROOT_NAME);
    set_body(&mut nodes, super_root, super_entries);
    set_body(&mut nodes, fpt, flat_path_table(&paths));

    // ---- layout -------------------------------------------------------------------------
    let per_block = block
        .checked_div(inode::PLAIN_SIZE)
        .filter(|n| *n > 0)
        .ok_or(PfsError::Malformed("a block holds no inodes"))?;
    let inode_blocks = nodes.len().div_ceil(per_block);

    // Block 0 is the superblock, the inode table follows it, then every node's data in the
    // order the inodes were numbered. An empty block sits after the flat path table, which is
    // what every real image leaves there.
    let mut next = u32::try_from(inode_blocks.checked_add(1).ok_or(PfsError::OutOfRange)?)
        .map_err(|_| PfsError::OutOfRange)?;
    for (index, node) in nodes.iter_mut().enumerate() {
        node.start = next;
        let blocks = u32::try_from(node.body.len().div_ceil(block).max(1))
            .map_err(|_| PfsError::OutOfRange)?;
        next = next.checked_add(blocks).ok_or(PfsError::OutOfRange)?;
        if index == 1 {
            next = next.checked_add(1).ok_or(PfsError::OutOfRange)?;
        }
    }
    let total = usize::try_from(next).map_err(|_| PfsError::OutOfRange)?;

    // ---- emit ---------------------------------------------------------------------------
    let mut out = vec![0_u8; total.checked_mul(block).ok_or(PfsError::OutOfRange)?];
    write_superblock(&mut out, block_size, nodes.len(), inode_blocks, next)?;

    for (index, node) in nodes.iter().enumerate() {
        let at = index
            .checked_mul(inode::PLAIN_SIZE)
            .and_then(|offset| offset.checked_add(block))
            .ok_or(PfsError::OutOfRange)?;
        write_inode(&mut out, at, node, block, index)?;

        let data = usize::try_from(node.start)
            .map_err(|_| PfsError::OutOfRange)?
            .checked_mul(block)
            .ok_or(PfsError::OutOfRange)?;
        let end = data
            .checked_add(node.body.len())
            .ok_or(PfsError::OutOfRange)?;
        out.get_mut(data..end)
            .ok_or(PfsError::OutOfRange)?
            .copy_from_slice(&node.body);
    }
    Ok(out)
}

/// Serialise one directory, adding inodes for everything inside it.
///
/// Depth-first, and the entries are built as it goes: a directory cannot be written until its
/// children have numbers, and a child has no number until it has been reached.
fn serialise_dir(
    dir: &Tree,
    self_inode: u32,
    parent_inode: u32,
    nodes: &mut Vec<Node>,
    prefix: &str,
    paths: &mut Vec<(String, u32, bool)>,
    depth: usize,
) -> Result<Vec<u8>, PfsError> {
    // The reader refuses a tree deeper than 64. Refusing to build one is better than building
    // an image that this crate's own reader will not walk.
    if depth > 64 {
        return Err(PfsError::Malformed("directory nesting is implausibly deep"));
    }
    let mut entries = Vec::new();
    // Every directory names itself and its parent first. A reader skips them; a filesystem
    // that omits them is one nothing can walk upwards through.
    dirent(&mut entries, self_inode, kind::DOT, ".");
    dirent(&mut entries, parent_inode, kind::DOT_DOT, "..");

    for (name, bytes) in &dir.files {
        let child = push(nodes, imode::FILE | imode::RX);
        set_body(nodes, child, bytes.clone());
        dirent(&mut entries, child, kind::FILE, name);
        paths.push((format!("{prefix}/{name}"), child, false));
    }
    for sub in &dir.dirs {
        let child = push(nodes, imode::DIR | imode::RX);
        dirent(&mut entries, child, kind::DIRECTORY, &sub.name);
        let path = format!("{prefix}/{}", sub.name);
        paths.push((path.clone(), child, true));
        let body = serialise_dir(
            sub,
            child,
            self_inode,
            nodes,
            &path,
            paths,
            depth.saturating_add(1),
        )?;
        set_body(nodes, child, body);
    }
    Ok(entries)
}

/// Append one directory entry.
///
/// Shared with [`crate::outer`]. It had a second copy that spelled the header size and the
/// alignment as bare numbers, with none of the reasoning below attached - the same rule twice,
/// once explained and once not. (D063)
pub(crate) fn dirent(out: &mut Vec<u8>, inode: u32, kind: u32, name: &str) {
    let name = name.as_bytes();
    // The name is followed by at least one byte of padding and the whole entry is aligned, so
    // a reader stepping by the recorded size lands on the next one.
    let size = DIRENT_HEADER
        .saturating_add(name.len())
        .saturating_add(1)
        .next_multiple_of(DIRENT_ALIGN);
    out.extend_from_slice(&inode.to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&u32::try_from(name.len()).unwrap_or_default().to_le_bytes());
    out.extend_from_slice(&u32::try_from(size).unwrap_or_default().to_le_bytes());
    out.extend_from_slice(name);
    let padded = out
        .len()
        .saturating_add(size.saturating_sub(DIRENT_HEADER.saturating_add(name.len())));
    out.resize(padded, 0);
}

/// Mark on a table value saying the entry is a directory.
const FPT_DIRECTORY: u32 = 0x2000_0000;

/// The flat path table: a hash of every path, mapped to the inode holding it.
///
/// A console uses this to reach a file without walking directories. **Nothing in this crate
/// reads it** - a reader follows directory entries, which are the authority - so a wrong table
/// here would pass every test and fail only on the machine it was built for. That is exactly
/// why it is built properly rather than left as a placeholder.
///
/// Entries are `(hash, value)` pairs of little-endian words, **sorted by hash**, where the
/// value is the inode number with [`FPT_DIRECTORY`] set for a directory.
///
/// # Collisions
///
/// Two paths hashing alike are resolved through a second file, `collision_resolver`, which the
/// super root names alongside this one. That is not built here: it has never been needed for a
/// tree this produces, and [`has_collision`] lets a caller find out before building rather
/// than after. [`build`] refuses rather than emitting a table with a silently lost entry.
fn flat_path_table(entries: &[(String, u32, bool)]) -> Vec<u8> {
    let mut pairs: Vec<(u32, u32)> = entries
        .iter()
        .map(|(path, inode, is_dir)| {
            let value = if *is_dir {
                *inode | FPT_DIRECTORY
            } else {
                *inode
            };
            (path_hash(path), value)
        })
        .collect();
    pairs.sort_unstable();

    let mut out = Vec::with_capacity(pairs.len().saturating_mul(8));
    for (hash, value) in pairs {
        out.extend_from_slice(&hash.to_le_bytes());
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// One path table entry, for a caller building a table of its own.
///
/// The outer filesystem has a fixed shape - one file - so it builds its table directly rather
/// than walking a tree it does not have. Sharing the hash matters more than sharing the walk:
/// two implementations of this hash that disagree produce two images that both read back fine
/// here and one of which a console cannot use.
#[must_use]
pub fn path_table_entry(path: &str, inode: u32, is_dir: bool) -> Vec<u8> {
    flat_path_table(&[(path.to_owned(), inode, is_dir)])
}

/// The hash the table is keyed by.
///
/// `hash = uppercase(c) + 31 * hash`, wrapping, over the path from the root - the same shape as
/// the familiar string hash, over an upper-cased path. The upper-casing is why two files
/// differing only in case collide, and it is ASCII-only here because a path in this filesystem
/// is bytes rather than text.
fn path_hash(path: &str) -> u32 {
    path.bytes().fold(0_u32, |hash, byte| {
        u32::from(byte.to_ascii_uppercase()).wrapping_add(hash.wrapping_mul(31))
    })
}

/// Whether any two paths in a tree hash alike.
///
/// Worth asking before building, because the answer decides whether an image needs a collision
/// resolver - which this does not write.
#[must_use]
pub fn has_collision(root: &Tree) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut paths = Vec::new();
    collect_paths(root, "", &mut paths);
    paths.iter().any(|path| !seen.insert(path_hash(path)))
}

/// Every path under a directory, as the table spells them.
fn collect_paths(dir: &Tree, prefix: &str, out: &mut Vec<String>) {
    for (name, _) in &dir.files {
        out.push(format!("{prefix}/{name}"));
    }
    for sub in &dir.dirs {
        let path = format!("{prefix}/{}", sub.name);
        out.push(path.clone());
        collect_paths(sub, &path, out);
    }
}

/// Add a node, returning its inode number.
fn push(nodes: &mut Vec<Node>, mode: u16) -> u32 {
    let number = u32::try_from(nodes.len()).unwrap_or_default();
    nodes.push(Node {
        mode,
        body: Vec::new(),
        start: 0,
    });
    number
}

/// Fill in a node's contents, now they are known.
fn set_body(nodes: &mut [Node], number: u32, body: Vec<u8>) {
    if let Some(node) = usize::try_from(number)
        .ok()
        .and_then(|index| nodes.get_mut(index))
    {
        node.body = body;
    }
}

/// Write the superblock into block zero.
fn write_superblock(
    out: &mut [u8],
    block_size: u32,
    inode_count: usize,
    inode_blocks: usize,
    total_blocks: u32,
) -> Result<(), PfsError> {
    put64(out, superblock::VERSION, 1)?;
    put64(out, superblock::MAGIC, crate::MAGIC)?;
    // Read-only, and the flag every image sets. Not signed and not encrypted: those describe
    // layers this does not build, and claiming either would send a reader looking for digests
    // that are not there and reading inodes at the wrong stride.
    if let Some(byte) = out.get_mut(superblock::READ_ONLY) {
        *byte = 1;
    }
    put16(out, superblock::MODE, mode::UNKNOWN_ALWAYS_SET)?;
    put32(out, superblock::BLOCK_SIZE, block_size)?;
    // Not the block count, despite the name a reader would guess. `LibOrbisPkg@6434772` writes a
    // literal `1` here and remarks that it is always 1; the field that actually sizes the image
    // is `N_DBLOCK`. Writing the real count here looked more correct and would have made every
    // image this builds differ from every image examined, in a field nothing checks.
    put64(out, superblock::N_BLOCK, 1)?;
    put64(
        out,
        superblock::INODE_COUNT,
        u64::try_from(inode_count).map_err(|_| PfsError::OutOfRange)?,
    )?;
    // The invariant the reader checks: this times the block size is the image length.
    put64(out, superblock::N_DBLOCK, u64::from(total_blocks))?;
    put64(
        out,
        superblock::INODE_BLOCKS,
        u64::try_from(inode_blocks).map_err(|_| PfsError::OutOfRange)?,
    )?;
    // The header embeds an inode describing the inode table itself. A console's mount reads it
    // to find the inodes, and this crate left it blank - which is what an inner image built here
    // presented, and the kernel's `nmount()` refused it `EINVAL` after the outer image mounted.
    // Every field below is set in a real inner image; the flag is `0x10` (the inner convention,
    // not the outer's `0x0C`) and the size is the inode table's own length. (measured)
    let table_len = u64::from(inode_blocks_u32(inode_blocks)?)
        .checked_mul(u64::from(block_size))
        .ok_or(PfsError::OutOfRange)?;
    put16(out, EMBEDDED_INODE + field::NLINK, 1)?;
    put32(out, EMBEDDED_INODE + field::FLAGS, iflag::BASE)?;
    put64(out, EMBEDDED_INODE + inode::SIZE, table_len)?;
    put64(out, EMBEDDED_INODE + field::SIZE_COMPRESSED, table_len)?;
    put32(
        out,
        EMBEDDED_INODE + inode::BLOCKS,
        inode_blocks_u32(inode_blocks)?,
    )?;

    // An unseeded image puts its `1` four bytes earlier than a seeded one does. The seeded
    // layout writes an index at `UNKNOWN_INDEX` and the seed after it; without a seed the
    // source writes `1` at `NO_SEED_INDEX` and stops. Both were measured as "a 1 near the end
    // of the header", and they are not the same field.
    // A `1` at `0xD8`, which this crate left zero and every real package sets.
    //
    // Reproduced, **not interpreted** - the same standing as the package manifest's `LEADING`.
    // All three real packages agree on it and they agree with each other everywhere else in this
    // region, so it is not a per-title value. `LibProsperoPKG@main` names the offset
    // `UnknownIndex` and does not explain it either, which is two independent readings calling it
    // unknown rather than one guess.
    //
    // Worth having because of *where* it is: this is the inner superblock, the structure a console
    // reads when it mounts `/app0`, and mounting the inner image is where a package built here
    // takes the machine down. Reproducing a field a real image sets is cheap; leaving it zero on
    // the grounds that nobody has named it is how the inode flags stayed wrong for three trips.
    put32(out, superblock::UNKNOWN_D8, 1)?;

    put32(out, superblock::NO_SEED_INDEX, 1)
}

/// The inode-block count as the `u32` the inode fields want it as.
fn inode_blocks_u32(inode_blocks: usize) -> Result<u32, PfsError> {
    u32::try_from(inode_blocks).map_err(|_| PfsError::OutOfRange)
}

/// Write one inode.
fn write_inode(
    out: &mut [u8],
    at: usize,
    node: &Node,
    block: usize,
    index: usize,
) -> Result<(), PfsError> {
    let blocks = node.body.len().div_ceil(block).max(1);
    // A directory's size is its entries rounded up to whole blocks; a file's is its own length.
    // The distinction is not cosmetic: a reader walks a directory by block and reads a file by
    // size, so a directory sized to its entries would have its last block truncated.
    let size = if node.mode & imode::DIR == 0 {
        node.body.len()
    } else {
        blocks.checked_mul(block).ok_or(PfsError::OutOfRange)?
    };
    let size = u64::try_from(size).map_err(|_| PfsError::OutOfRange)?;

    // The first two inodes are the super root and the path table, which are internal to the
    // filesystem; every other inode is an ordinary file or directory. A console's mount reads
    // these flags and refuses an inner image whose inodes carry none.
    let flags = if index < 2 {
        iflag::BASE | iflag::INTERNAL
    } else {
        iflag::BASE
    };

    put16(out, at, node.mode)?;
    put16(out, offset(at, field::NLINK)?, 1)?;
    put32(out, offset(at, field::FLAGS)?, flags)?;
    put64(out, offset(at, inode::SIZE)?, size)?;
    put64(out, offset(at, field::SIZE_COMPRESSED)?, size)?;
    put32(
        out,
        offset(at, inode::BLOCKS)?,
        u32::try_from(blocks).map_err(|_| PfsError::OutOfRange)?,
    )?;
    put32(out, offset(at, inode::PLAIN_START)?, node.start)
}

fn offset(base: usize, field: usize) -> Result<usize, PfsError> {
    base.checked_add(field).ok_or(PfsError::OutOfRange)
}

fn put16(out: &mut [u8], at: usize, value: u16) -> Result<(), PfsError> {
    put(out, at, &value.to_le_bytes())
}

fn put32(out: &mut [u8], at: usize, value: u32) -> Result<(), PfsError> {
    put(out, at, &value.to_le_bytes())
}

fn put64(out: &mut [u8], at: usize, value: u64) -> Result<(), PfsError> {
    put(out, at, &value.to_le_bytes())
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
    use super::{ROOT_INODE, ROOT_NAME, Tree, build};
    use crate::{Filesystem, Slice, Superblock};

    const BLOCK: u32 = 0x10000;

    #[test]
    fn an_image_this_crate_builds_is_one_it_can_read() {
        // The whole claim in one test: every offset, count and block number is exercised by
        // reading the result back, and a wrong one surfaces as a missing file or wrong bytes.
        let tree = Tree::new(ROOT_NAME)
            .with_file("eboot.bin", vec![0xAB; 1000])
            .with_dir(
                Tree::new("sce_module")
                    .with_file("libc.prx", vec![0xCD; 5000])
                    .with_file("libSceFios2.prx", b"small".to_vec()),
            );
        let image = build(&tree, BLOCK).expect("an image");

        let sb = Superblock::parse(&image).expect("a superblock");
        assert_eq!(sb.block_size, BLOCK);
        assert!(!sb.is_signed(), "this builds plain images");
        assert!(!sb.is_encrypted(), "and unencrypted ones");
        assert_eq!(
            sb.image_len(),
            u64::try_from(image.len()).unwrap(),
            "the block count must describe the image it is inside"
        );

        let fs = Filesystem::new(Slice::new(&image, 0)).expect("a filesystem");
        let found = fs.walk(ROOT_INODE).expect("a walk from the root");
        let mut paths: Vec<&str> = found.iter().map(|f| f.path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(
            paths,
            [
                "/eboot.bin",
                "/sce_module/libSceFios2.prx",
                "/sce_module/libc.prx"
            ]
        );
    }

    #[test]
    fn every_file_reads_back_with_the_bytes_it_went_in_with() {
        // Larger than one block, so the block count and the multi-block read are exercised
        // rather than assumed. Empty, so a zero-length file does not read back as a block of
        // padding.
        let big: Vec<u8> = (0..70_000_u32)
            .map(|n| u8::try_from(n & 0xFF).unwrap())
            .collect();
        let tree = Tree::new(ROOT_NAME)
            .with_file("small", b"hello".to_vec())
            .with_file("empty", Vec::new())
            .with_file("big", big.clone());
        let image = build(&tree, BLOCK).expect("an image");

        let fs = Filesystem::new(Slice::new(&image, 0)).expect("a filesystem");
        let found = fs.walk(ROOT_INODE).expect("a walk");
        assert_eq!(found.len(), 3, "three files went in");
        for entry in found {
            let bytes = fs.contents(entry.inode).expect("file bytes");
            let want: &[u8] = match entry.path.as_str() {
                "/small" => b"hello",
                "/empty" => b"",
                "/big" => &big,
                other => panic!("unexpected {other}"),
            };
            assert_eq!(bytes, want, "{} differs", entry.path);
        }
    }

    #[test]
    fn the_roots_parent_is_itself_and_not_the_super_root() {
        // `outer.rs` states this rule for the filesystem it builds and this one did the obvious
        // thing instead, so the two filesystems in one package disagreed about a shared structure.
        // Three real packages have `uroot`'s `..` naming `uroot`; this named the super root, and
        // anything walking up from the mount left the tree rather than staying at its top.
        //
        // Checked in the bytes rather than through the reader, which skips `.` and `..`. (D072)
        let image = build(&Tree::new(ROOT_NAME), BLOCK).expect("an image");
        let block = BLOCK as usize;

        // The root is inode 2; its first data block is recorded at `PLAIN_START` in its inode.
        let inode = block + ROOT_INODE * super::inode::PLAIN_SIZE;
        let at = super::inode::PLAIN_START;
        let start = u32::from_le_bytes(
            image[inode + at..inode + at + 4]
                .try_into()
                .expect("four bytes"),
        ) as usize;

        // The second entry of a directory block is always `..`; each entry declares its own size.
        let base = start * block;
        let first_size =
            u32::from_le_bytes(image[base + 12..base + 16].try_into().expect("four bytes"))
                as usize;
        let dotdot = base + first_size;
        let parent = u32::from_le_bytes(image[dotdot..dotdot + 4].try_into().expect("four bytes"));
        let name_len = u32::from_le_bytes(
            image[dotdot + 8..dotdot + 12]
                .try_into()
                .expect("four bytes"),
        ) as usize;
        let name = &image[dotdot + 16..dotdot + 16 + name_len];

        assert_eq!(name, b"..", "the second entry should be the parent");
        assert_eq!(
            parent as usize, ROOT_INODE,
            "the root's parent must be the root, not the super root"
        );
    }

    #[test]
    fn the_super_root_names_the_root_and_the_path_table() {
        // Inode 0 is the super root, and what it lists is the entry point to everything else.
        let image = build(&Tree::new(ROOT_NAME), BLOCK).expect("an image");
        let fs = Filesystem::new(Slice::new(&image, 0)).expect("a filesystem");
        let found = fs.walk(0).expect("a walk from the super root");
        let paths: Vec<&str> = found.iter().map(|f| f.path.as_str()).collect();
        assert!(
            paths.contains(&"/flat_path_table"),
            "the super root names the path table: {paths:?}"
        );
    }

    #[test]
    fn nesting_survives_the_round_trip() {
        let tree = Tree::new(ROOT_NAME)
            .with_dir(Tree::new("a").with_dir(
                Tree::new("b").with_dir(Tree::new("c").with_file("deep", b"x".to_vec())),
            ));
        let image = build(&tree, BLOCK).expect("an image");
        let fs = Filesystem::new(Slice::new(&image, 0)).expect("a filesystem");
        let found = fs.walk(ROOT_INODE).expect("a walk");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, "/a/b/c/deep");
    }

    #[test]
    fn a_block_too_small_for_an_inode_is_refused_rather_than_silently_wrong() {
        assert!(build(&Tree::new(ROOT_NAME), 0).is_err());
        assert!(
            build(&Tree::new(ROOT_NAME), 0x40).is_err(),
            "an inode is 0xA8, so nothing fits"
        );
    }
}
