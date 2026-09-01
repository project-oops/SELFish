//! Does this crate reproduce a real package's `sce_sys/keystone`, byte for byte?
//!
//! The keystone was missing from every package this crate built, because it looked like one
//! more file the caller owns. It is not - it is two `HMAC-SHA256` operations over the
//! passcode, which the builder already has. Every real package carries one.
//!
//! This opens the filesystem inside a package, pulls the keystone out, and compares it against
//! one computed from the same passcode. A package built with a passcode nobody can recover
//! will differ, and that is reported as such rather than as a failure.
//!
//! ```text
//! cargo run -p selfish-pkg --example keystone -- <package>...
//! ```

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "an example reads better with plain arithmetic; the library it drives does not"
)]

use selfish_pfs::{Compressed, Filesystem, Region, Slice, Xts, outer, write as pfs};
use selfish_pkg::{Package, keys, keystone};

/// Sectors at the front of an image that are not encrypted.
const PLAIN_SECTORS: u64 = 16;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: keystone <package>...");
        std::process::exit(2);
    }

    let mut matched = 0_usize;
    let mut compared = 0_usize;
    for path in &paths {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let bytes = std::fs::read(path)?;
        let package = Package::parse(&bytes)?;
        println!("{name}");

        let Ok(key) = keys::filesystem_key(&package) else {
            println!("  not readable as a fake package");
            continue;
        };
        let found = match extract_keystone(&bytes, &package, &key) {
            Ok(Some(found)) => found,
            Ok(None) => {
                println!("  carries no keystone");
                continue;
            }
            Err(error) => {
                println!("  could not open the filesystem: {error}");
                continue;
            }
        };
        println!("  found  {} bytes", found.len());

        // Only a package built with the fake passcode can be reproduced; the passcode is an
        // input and nothing recovers it from a finished package.
        let computed = keys::derive_filesystem_key(package.content_id(), keys::FAKE_PASSCODE);
        if key != computed {
            println!("  built with another passcode, so its keystone cannot be reproduced");
            continue;
        }
        compared += 1;

        let ours = keystone::create(keys::FAKE_PASSCODE)?;
        if ours == found {
            matched += 1;
            println!("  {} bytes identical", ours.len());
        } else {
            let differing = found.iter().zip(&ours).filter(|(a, b)| a != b).count();
            println!("  {differing} of {} bytes differ", ours.len());
            // Which third disagrees says which step is wrong: the header is fixed, the
            // fingerprint is the passcode MAC, and the last is the MAC over the first two.
            for (label, range) in [
                ("header     ", 0..32),
                ("fingerprint", 32..64),
                ("final MAC  ", 64..96),
            ] {
                let same = found.get(range.clone()) == ours.get(range);
                println!("    {label} {}", if same { "match" } else { "DIFFER" });
            }
        }
    }

    println!();
    println!("{matched} of {compared} fake-passcode package(s) reproduced exactly");
    if compared > 0 && matched < compared {
        std::process::exit(1);
    }
    Ok(())
}

/// Open the filesystem inside a package and read the keystone out of it.
fn extract_keystone(
    bytes: &[u8],
    package: &Package<'_>,
    key: &[u8],
) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
    let at = package.image_offset()?;
    let start = usize::try_from(at)?;
    let header = &bytes[start..start + 0x400];
    let (tweak, data) = selfish_pfs::image_keys(key, header)?;
    let length = u64::try_from(bytes.len())? - at;

    let outer_fs = Filesystem::new(Xts::new(
        Region::new(Slice::new(bytes, 0), at, length),
        &tweak,
        &data,
        PLAIN_SECTORS,
    )?)?;
    let image = outer_fs
        .walk(pfs::ROOT_INODE)?
        .into_iter()
        .find(|found| found.path.ends_with(outer::IMAGE_NAME))
        .ok_or("no image file in the outer filesystem")?;
    let container = outer_fs.contents(image.inode)?;

    let inner = Filesystem::new(Compressed::new(Slice::new(&container, 0))?)?;
    for found in inner.walk(pfs::ROOT_INODE)? {
        if found.path.ends_with("keystone") {
            return Ok(Some(inner.contents(found.inode)?));
        }
    }
    Ok(None)
}
