//! `pkg`, `extract` and `derive`: reading packages.

use std::path::{Path, PathBuf};

use selfish_pfs::{Compressed, Filesystem, Region, Slice, Source, Xts};

use crate::Result;

/// The decryption layer over a package's image, with what its superblock said.
struct Image<'a> {
    decrypted: Box<Xts<Slice<'a>>>,
    block_size: u64,
    /// Where the image begins in the package.
    offset: u64,
}

/// A package's files, and the inner filesystem that holds them. `pkg` and `extract` both reach
/// the files through this one path.
struct Opened<'a> {
    files: Vec<selfish_pfs::Found>,
    inner: Filesystem<Compressed<Region<&'a Xts<Slice<'a>>>>>,
}

/// Decrypt a package's image. The superblock is in the clear, and carries the seed the image
/// keys are derived from.
fn open(bytes: &[u8]) -> Result<Image<'_>> {
    let package = selfish_pkg::Package::parse(bytes)?;
    let key = selfish_pkg::keys::filesystem_key(&package)?;
    let at = package.image_offset()?;

    let image = Slice::new(bytes, at);
    let superblock = image.read(0, 0x400)?;
    let block_size = u64::from(selfish_bytes::read_le::<u32>(&superblock, 0x20).unwrap_or(0));
    if block_size == 0 {
        return Err("the image superblock declares a zero block size".into());
    }
    let (tweak, data) = selfish_pfs::image_keys(&key, &superblock)?;
    let sectors = block_size
        .checked_div(selfish_pfs::SECTOR_SIZE)
        .unwrap_or(0);
    Ok(Image {
        decrypted: Box::new(Xts::new(image, &tweak, &data, sectors)?),
        block_size,
        offset: at,
    })
}

/// Walk from the decrypted outer filesystem to the files of the inner one.
///
/// The inner image is the largest file in the outer filesystem, found by size rather than by a
/// fixed block number, because packages do not agree on the block.
fn walk<'a>(decrypted: &'a Xts<Slice<'a>>) -> Result<Opened<'a>> {
    let outer = Filesystem::new(decrypted)?;
    let biggest = outer
        .inodes()
        .iter()
        .max_by_key(|inode| inode.size)
        .copied()
        .ok_or("the outer filesystem is empty")?;
    let window = Region::new(
        // `source()` is a reference to the reference the filesystem was built from.
        *outer.source(),
        u64::from(biggest.start).saturating_mul(outer.block_size()),
        biggest.size,
    );
    let inner = Filesystem::new(Compressed::new(window)?)?;
    let files = inner.walk(0)?;
    Ok(Opened { files, inner })
}

/// `selfish pkg`: the entries, the image, and the first forty files (or all of them).
pub(crate) fn list(path: &Path, all: bool) -> Result {
    let bytes = std::fs::read(path)?;
    let package = selfish_pkg::Package::parse(&bytes)?;
    say!("entries    {}", package.entries().len());
    let missing = package.missing_expected_entries();
    if missing.is_empty() {
        say!("           every expected entry present");
    } else {
        say!("           MISSING {missing:#x?}");
    }

    let image = open(&bytes)?;
    say!("image at   {:#x}", image.offset);
    say!("block size {:#x}", image.block_size);
    let opened = walk(&image.decrypted)?;
    say!("files      {}", opened.files.len());

    let shown = if all { opened.files.len() } else { 40 };
    for found in opened.files.iter().take(shown) {
        let size = opened.inner.inodes().get(found.inode).map_or(0, |i| i.size);
        say!("  {size:>12}  {}", found.path);
    }
    if opened.files.len() > shown {
        say!(
            "  ... and {} more",
            opened.files.len().saturating_sub(shown)
        );
    }
    Ok(())
}

/// `selfish extract`: every file under `out`.
///
/// A path that would climb out of `out` is refused rather than sanitised: a filesystem that
/// describes `../..` is a finding, not something to correct quietly.
pub(crate) fn extract(path: &Path, out: &Path) -> Result {
    let bytes = std::fs::read(path)?;
    let image = open(&bytes)?;
    let opened = walk(&image.decrypted)?;

    let mut written = 0_usize;
    for found in &opened.files {
        if found.path.contains("..") {
            eprintln!(
                "refusing {}: the path would escape the destination",
                found.path
            );
            continue;
        }
        let target = out.join(found.path.trim_start_matches('/'));
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, opened.inner.contents(found.inode)?)?;
        written = written.saturating_add(1);
    }
    say!("{written} files written to {}", out.display());
    Ok(())
}

/// `selfish derive`: re-run the entry derivations against the packages given.
pub(crate) fn derive(paths: &[PathBuf]) -> Result {
    if paths.is_empty() {
        return Err("give it some packages: selfish derive a.pkg b.pkg ...".into());
    }

    let mut bytes = Vec::new();
    for path in paths {
        bytes.push((path.clone(), std::fs::read(path)?));
    }
    let mut packages = Vec::new();
    for (path, raw) in &bytes {
        match selfish_pkg::Package::parse(raw) {
            Ok(package) => packages.push(package),
            Err(error) => say!("skipped {}: {error}", path.display()),
        }
    }

    let derivation = selfish_pkg::derive::run(&packages);
    say!("samples    {}", derivation.samples);
    if derivation.samples < 2 {
        say!("  a single package cannot distinguish a format from a coincidence");
    }
    say!();

    for finding in &derivation.findings {
        say!(
            "entry {:#06x}  {}/{} packages agree{}",
            finding.entry,
            finding.held,
            finding.tested,
            if finding.survived() {
                ""
            } else {
                "  <-- FAILED"
            }
        );
        say!("  claim    {}", finding.claim);
        for note in &finding.notes {
            say!("  note     {note}");
        }
    }

    say!();
    if derivation.is_consistent() {
        say!("every claim survived every package it could be tested on");
        Ok(())
    } else {
        say!("a claim failed: the table in data/pkg-format.tsv is wrong, or this package is");
        say!("built by something that disagrees. Either is worth knowing.");
        Err("derivation inconsistent".into())
    }
}
