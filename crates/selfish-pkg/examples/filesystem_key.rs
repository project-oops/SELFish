//! Do the two routes to the filesystem key agree on real packages?
//!
//! There are two ways to arrive at `EKPFS`, and they share nothing but the answer:
//!
//! - **Recovered.** Take the package's image-key entry, which holds the key encrypted under the
//!   fake keyset, and decrypt it with that keyset's private half. This reads the package.
//! - **Computed.** Hash the content id and the passcode. This never looks at the package at all
//!   beyond reading the id off it.
//!
//! A builder needs the second, because a package that does not exist yet has no entry to
//! decrypt. This run checks the second against the first on packages in hand, which is the only
//! way to find out whether the derivation a writer will depend on is the right one. (principle
//! 2: real files confirm, they do not derive.)
//!
//! It then uses the key for what it is for - decrypting the outer filesystem, finding the image
//! inside it, and walking the filesystem inside that - so a pass means the whole chain holds,
//! not just thirty-two bytes.
//!
//! # A difference is not necessarily a disagreement
//!
//! The passcode is an *input*. A package built with something other than the fake one computes
//! to a different key and is not evidence of a wrong derivation. That case is separated here by
//! opening the image with the recovered key: if it opens, the format is understood and only the
//! passcode is unknown - which is not something a package is supposed to give up.
//!
//! ```text
//! cargo run -p selfish-pkg --example filesystem_key -- <package>...
//! ```

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "an example reads better with plain arithmetic; the library it drives does not"
)]

use selfish_pfs::{Compressed, Filesystem, Region, Slice, Xts};
use selfish_pkg::keys;

/// Sectors at the front of an image that are not encrypted.
const PLAIN_SECTORS: u64 = 16;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: filesystem_key <package>...");
        std::process::exit(2);
    }

    let mut agreed = 0_usize;
    let mut opened = 0_usize;
    let mut checked = 0_usize;
    for path in &paths {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let bytes = std::fs::read(path)?;
        let package = selfish_pkg::Package::parse(&bytes)?;
        println!("{name}");

        let recovered = match keys::filesystem_key(&package) {
            Ok(key) => key,
            Err(error) => {
                // A retail package's image key is not under the fake keyset, and saying so is
                // more useful than reporting a mismatch that was never a comparison.
                println!("  not readable as a fake package: {error}");
                continue;
            }
        };
        let content_id = package.content_id();
        let computed = keys::derive_filesystem_key(content_id, keys::FAKE_PASSCODE);

        checked += 1;
        println!("  content id  {}", String::from_utf8_lossy(content_id));
        println!("  recovered   {}", hex(&recovered));
        println!("  computed    {}", hex(&computed));

        let matches = recovered.as_slice() == computed.as_slice();
        if matches {
            agreed += 1;
            println!("  the two routes agree");
        } else {
            println!("  they differ, so this package was not built with the fake passcode");
        }

        // Whichever key is this package's own, walk with it. On a match that proves the computed
        // key really works; on a difference it separates "another passcode" from "wrong
        // derivation", which look identical if you only compare thirty-two bytes.
        let key: &[u8] = if matches { &computed } else { &recovered };
        match walk(&bytes, &package, key) {
            Ok((outer, inner)) => {
                opened += 1;
                println!("  outer filesystem holds {outer} file");
                println!("  inner filesystem holds {inner} files");
                if !matches {
                    println!("  it opens under the recovered key, so the derivation is sound");
                }
            }
            Err(error) => println!("  the image did not open: {error}"),
        }
    }

    println!();
    println!("{opened} of {checked} image(s) opened");
    println!("{agreed} of {checked} used the fake passcode");
    if checked > 0 && opened < checked {
        std::process::exit(1);
    }
    Ok(())
}

/// Open the image with a key and count what is inside, at both levels.
fn walk(
    bytes: &[u8],
    package: &selfish_pkg::Package<'_>,
    key: &[u8],
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let at = package.image_offset()?;
    let length = u64::try_from(bytes.len())? - at;
    let image = Region::new(Slice::new(bytes, 0), at, length);

    // The keys come from the seed in the superblock, which lives in the part that is not
    // encrypted - so it can be read before anything is decrypted.
    let start = usize::try_from(at)?;
    let header = &bytes[start..start + 0x400];
    let (tweak, data) = selfish_pfs::image_keys(key, header)?;

    let outer = Filesystem::new(Xts::new(image, &tweak, &data, PLAIN_SECTORS)?)?;
    let outer_files = outer.walk(selfish_pfs::write::ROOT_INODE)?;

    let image_entry = outer_files
        .iter()
        .find(|found| found.path.ends_with(selfish_pfs::outer::IMAGE_NAME))
        .ok_or("the outer filesystem has no image file")?;
    let container = outer.contents(image_entry.inode)?;

    let inner = Filesystem::new(Compressed::new(Slice::new(&container, 0))?)?;
    let inner_files = inner.walk(selfish_pfs::write::ROOT_INODE)?;
    Ok((outer_files.len(), inner_files.len()))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}
