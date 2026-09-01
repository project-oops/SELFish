//! List what is inside a real package.
//!
//! The whole chain in one run: outer container, key derivation, encrypted filesystem, the
//! compressed image inside it, and the filesystem inside *that*. Nothing short of this
//! exercises the nesting, and the nesting is the format.
//!
//! ```text
//! cargo run -p selfish-pkg --example list -- <package>
//! ```

#![allow(
    clippy::arithmetic_side_effects,
    reason = "an example reads better with plain arithmetic; the library it drives does not"
)]

use selfish_pfs::{Compressed, Filesystem, Region, Slice, Source, Xts};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: list <package>");
        std::process::exit(2);
    };

    let bytes = std::fs::read(&path)?;
    let package = selfish_pkg::Package::parse(&bytes)?;
    let key = selfish_pkg::keys::filesystem_key(&package)?;
    let at = package.image_offset()?;
    println!("filesystem key: {} bytes, image at {at:#x}", key.len());

    // The superblock is in the clear even though the rest of the image is not - which is
    // what makes the key derivation below possible, since it carries the seed.
    let image = Slice::new(&bytes, at);
    let superblock = image.read(0, 0x400)?;
    let block_size = u64::from(u32::from_le_bytes([
        *superblock.get(0x20).unwrap_or(&0),
        *superblock.get(0x21).unwrap_or(&0),
        *superblock.get(0x22).unwrap_or(&0),
        *superblock.get(0x23).unwrap_or(&0),
    ]));
    let mode = u16::from_le_bytes([
        *superblock.get(0x1C).unwrap_or(&0),
        *superblock.get(0x1D).unwrap_or(&0),
    ]);
    println!("outer: block size {block_size:#x}, mode {mode:#x}");

    let (tweak, data) = selfish_pfs::image_keys(&key, &superblock)?;
    let decrypted = Xts::new(image, &tweak, &data, block_size / selfish_pfs::SECTOR_SIZE)?;
    let outer = Filesystem::new(decrypted)?;
    println!("outer filesystem: {} inodes", outer.inodes().len());

    // The inner image is simply the largest file in the outer filesystem. Picking it that
    // way rather than by a hardcoded block number, because a previous-generation extractor
    // hardcodes block 11 and these packages put it at block 7.
    let (index, biggest) = outer
        .inodes()
        .iter()
        .enumerate()
        .max_by_key(|(_, inode)| inode.size)
        .ok_or("the outer filesystem is empty")?;
    println!(
        "  inner image: inode {index}, {} bytes at block {}",
        biggest.size, biggest.start
    );

    let window = Region::new(
        outer.source(),
        u64::from(biggest.start) * outer.block_size(),
        biggest.size,
    );
    let inner = Filesystem::new(Compressed::new(window)?)?;
    println!("inner filesystem: {} inodes", inner.inodes().len());

    let files = inner.walk(0)?;
    println!("{} files:", files.len());
    for found in files.iter().take(40) {
        let size = inner.inodes().get(found.inode).map_or(0, |i| i.size);
        println!("  {:>12}  {}", size, found.path);
    }
    if files.len() > 40 {
        println!("  ... and {} more", files.len() - 40);
    }

    // The strongest check available on the container reader: a real one, produced by the
    // vendor toolchain, pulled out of a real package. Everything it has been tested against
    // until now was written by its own builder.
    // Which generation do real files actually use? Worth counting rather than assuming: the
    // one eboot examined by hand reported the *previous* generation, on a current-generation
    // package, which is either a general truth or a property of that one title.
    let mut current = 0_u32;
    let mut previous = 0_u32;
    let mut plain = 0_u32;
    for found in &files {
        let Ok(raw) = inner.contents(found.inode) else {
            continue;
        };
        match selfish_container::Container::parse(&raw) {
            Ok(c) if c.generation() == selfish_abi::Generation::Current => current += 1,
            Ok(_) => previous += 1,
            Err(_) => plain += 1,
        }
    }
    println!("containers: {current} current generation, {previous} previous, {plain} neither");

    if let Some(found) = files.iter().find(|f| f.path.ends_with("eboot.bin")) {
        let raw = inner.contents(found.inode)?;
        println!(
            "
{} ({} bytes):",
            found.path,
            raw.len()
        );
        match selfish_container::Container::parse(&raw) {
            Ok(container) => {
                println!("  {} container", container.generation());
                println!("  {} entries", container.entries().len());
                println!(
                    "  header {:#x}, metadata {:#x}",
                    container.header_size(),
                    container.meta_size()
                );
                match container.inner_elf_header() {
                    Ok(_) => println!("  inner executable at {:#x}", container.inner_offset()),
                    Err(error) => println!("  inner executable: {error}"),
                }
            }
            Err(error) => println!("  {error}"),
        }
    }
    Ok(())
}
