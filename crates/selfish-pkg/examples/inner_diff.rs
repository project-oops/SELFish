//! Compare the inner pfs of two packages - the filesystem a console mounts as `/app0`.
//!
//! With the outer inodes fixed the console gets past opening `pfs_image.dat` and now calls
//! `nmount()` on the inner image, which fails `EINVAL` on a package this crate built. The inner
//! image is `pfs_image.dat`, `PFSC`-compressed inside the outer filesystem. This decompresses it
//! for two packages and prints the inner superblock and inodes, so whatever the kernel rejects
//! is visible against a real one.
//!
//!     inner_diff <ours.pkg> <real.pkg>
// A diagnostic probe, held to a probe's standards rather than the library's.
//
// These read structures whose layout is already known, at offsets the format fixes, and print
// what they find. Indexing, slicing and plain arithmetic over those offsets is the clearest way
// to say what is being read - a probe that wraps every field access in a fallible conversion is
// harder to check against a hex dump, which is the only thing it will ever be checked against.
// Nothing here ships: a wrong offset produces a wrong line on a terminal, not a wrong package.
//
// The library itself keeps every one of these lints. This block is the boundary between the two.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::format_collect,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::doc_markdown,
    clippy::missing_panics_doc,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_lines
)]
use selfish_pfs::{Compressed, Filesystem, Slice, Source, Superblock, Xts};

fn inner_bytes(pkg: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let package = selfish_pkg::Package::parse(pkg)?;
    let key = selfish_pkg::keys::filesystem_key(&package)?;
    let at = package.image_offset()?;
    let image = Slice::new(pkg, at);
    let sb_raw = image.read(0, 0x400)?;
    let sb = Superblock::parse(&sb_raw)?;
    let block_size = sb.block_size as u64;
    let (tweak, data) = selfish_pfs::image_keys(&key, &sb_raw)?;
    let sectors = block_size / selfish_pfs::SECTOR_SIZE;
    let decrypted = Xts::new(image, &tweak, &data, sectors)?;
    let outer = Filesystem::new(&decrypted)?;
    // The one large file in the outer is pfs_image.dat, the PFSC container.
    let mut pfsc = Vec::new();
    for found in outer.walk(0).unwrap_or_default() {
        if found.path.ends_with("pfs_image.dat") {
            pfsc = outer.contents(found.inode)?;
        }
    }
    if pfsc.is_empty() {
        return Err("no pfs_image.dat in the outer filesystem".into());
    }
    // PFSC -> the inner pfs image. The PFSC header carries the decompressed length at 0x28.
    let data_len = {
        let mut v = [0u8; 8];
        v.copy_from_slice(pfsc.get(0x28..0x30).ok_or("PFSC too short")?);
        usize::try_from(u64::from_le_bytes(v)).unwrap_or(0)
    };
    let src = Compressed::new(Slice::new(&pfsc, 0))?;
    let inner = src.read(0, data_len)?;
    Ok(inner)
}

fn dump(label: &str, inner: &[u8]) {
    println!("\n===== {label}  ({} bytes) =====", inner.len());
    let sb = match Superblock::parse(inner) {
        Ok(sb) => sb,
        Err(err) => {
            println!("  inner superblock will not parse: {err:?}");
            let head: String = inner.iter().take(64).map(|b| format!("{b:02x}")).collect();
            println!("  first 64: {head}");
            return;
        }
    };
    println!(
        "  version={} magic-ok mode={:#06x} block_size={:#x} read_only={} inode_count={} \
         inode_blocks={} data_blocks={}",
        sb.version,
        sb.mode,
        sb.block_size,
        sb.read_only,
        sb.inode_count,
        sb.inode_blocks,
        sb.data_blocks
    );
    // Whether the inodes are signed (0x2C8 stride) or plain (0xA8) follows from the mode's
    // signed bit; print the stride the mode implies and the first inode's raw head either way.
    let signed = sb.mode & 0x1 != 0;
    let stride = if signed { 0x2C8 } else { 0xA8 };
    println!("  mode signed-bit={} -> inode stride {stride:#x}", signed);
    let block = sb.block_size as usize;
    for i in 0..sb.inode_count.min(6) as usize {
        let at = block + i * stride;
        if at + 0x68 > inner.len() {
            break;
        }
        let u16at = |o: usize| u16::from_le_bytes([inner[at + o], inner[at + o + 1]]);
        let u32at = |o: usize| {
            u32::from_le_bytes([
                inner[at + o],
                inner[at + o + 1],
                inner[at + o + 2],
                inner[at + o + 3],
            ])
        };
        let u64at = |o: usize| {
            let mut v = [0u8; 8];
            v.copy_from_slice(&inner[at + o..at + o + 8]);
            u64::from_le_bytes(v)
        };
        println!(
            "    [{i}] mode={:#06x} nlink={} flags={:#010x} size={} blocks={} start={:#x}",
            u16at(0x00),
            u16at(0x02),
            u32at(0x04),
            u64at(0x08),
            u32at(0x60),
            u32at(0x64),
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ours = std::env::args()
        .nth(1)
        .expect("usage: inner_diff <ours> <real>");
    let real = std::env::args()
        .nth(2)
        .expect("usage: inner_diff <ours> <real>");
    let oi = inner_bytes(&std::fs::read(&ours)?)?;
    let ri = inner_bytes(&std::fs::read(&real)?)?;
    dump("OURS inner", &oi);
    dump("REAL inner", &ri);

    // Raw superblock byte diffs (first 0x400), the fields the kernel's nmount validates. Size
    // fields legitimately differ; a flag or a constant that differs is the answer.
    println!("\n=== INNER SUPERBLOCK raw diffs (offset: ours real) ===");
    let mut any = false;
    for off in 0..0x400usize {
        let ob = oi.get(off).copied().unwrap_or(0);
        let rb = ri.get(off).copied().unwrap_or(0);
        if ob != rb {
            any = true;
            println!("   {off:#05x}: {ob:#04x} {rb:#04x}");
        }
    }
    if !any {
        println!("   (none)");
    }
    Ok(())
}
