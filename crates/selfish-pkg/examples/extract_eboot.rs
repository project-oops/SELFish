//! Extract `eboot.bin` from a real package to a file, for comparing its import table
//! against one this project builds. A diagnostic probe; nothing it writes ships.
//!
//! ```text
//! cargo run -p selfish-pkg --example extract_eboot -- <package> <out.bin>
//! ```

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used
)]

use selfish_pfs::{Compressed, Filesystem, Region, Slice, Source, Xts};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: extract_eboot <package> <out.bin>")?;
    let out = std::env::args()
        .nth(2)
        .ok_or("usage: extract_eboot <package> <out.bin>")?;

    let bytes = std::fs::read(&path)?;
    let package = selfish_pkg::Package::parse(&bytes)?;
    let key = selfish_pkg::keys::filesystem_key(&package)?;
    let at = package.image_offset()?;

    let image = Slice::new(&bytes, at);
    let superblock = image.read(0, 0x400)?;
    let block_size = u64::from(u32::from_le_bytes([
        *superblock.get(0x20).unwrap_or(&0),
        *superblock.get(0x21).unwrap_or(&0),
        *superblock.get(0x22).unwrap_or(&0),
        *superblock.get(0x23).unwrap_or(&0),
    ]));

    let (tweak, data) = selfish_pfs::image_keys(&key, &superblock)?;
    let decrypted = Xts::new(image, &tweak, &data, block_size / selfish_pfs::SECTOR_SIZE)?;
    let outer = Filesystem::new(decrypted)?;

    let (_index, biggest) = outer
        .inodes()
        .iter()
        .enumerate()
        .max_by_key(|(_, inode)| inode.size)
        .ok_or("the outer filesystem is empty")?;

    let window = Region::new(
        outer.source(),
        u64::from(biggest.start) * outer.block_size(),
        biggest.size,
    );
    let inner = Filesystem::new(Compressed::new(window)?)?;

    let files = inner.walk(0)?;
    let found = files
        .iter()
        .find(|f| f.path.ends_with("eboot.bin"))
        .ok_or("no eboot.bin in the package")?;
    let raw = inner.contents(found.inode)?;
    std::fs::write(&out, &raw)?;
    eprintln!("wrote {} bytes from {} -> {}", raw.len(), found.path, out);
    Ok(())
}
