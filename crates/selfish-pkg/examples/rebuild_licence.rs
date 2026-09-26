//! Rebuild each package's licence from its content id and title kind with
//! `licence::Licence::build`, and compare it with the real one byte for byte.
//!
//! ```text
//! cargo run -p selfish-pkg --example rebuild_licence -- <package>...
//! ```

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "an example reads better with plain arithmetic; the library it drives does not"
)]

use selfish_pkg::{Package, keys, licence};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: rebuild_licence <package>...");
        std::process::exit(2);
    }

    let mut all = true;
    for path in &paths {
        let bytes = std::fs::read(path)?;
        let Ok(package) = Package::parse(&bytes) else {
            continue;
        };
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);

        let Some(real) = package
            .entry(0x400)
            .and_then(|entry| keys::decrypt_entry(&package, entry).ok())
        else {
            println!("{name}: no readable licence");
            continue;
        };

        // The only things taken from the real licence: what the title *is*. A builder cannot
        // know those and this is not trying to guess them.
        let be16 = |at: usize| u16::from_be_bytes([real[at], real[at + 1]]);
        let built = licence::Licence::build(
            package.content_id(),
            be16(licence::field::DRM_TYPE),
            be16(licence::field::CONTENT_TYPE),
            be16(licence::field::SKU_FLAG),
        )?;

        if built.bytes == real {
            println!("{name}: REBUILT BYTE FOR BYTE");
            continue;
        }
        all = false;
        println!("{name}: differs");
        let mut shown = 0;
        for (at, (a, b)) in built.bytes.iter().zip(&real).enumerate() {
            if a != b && shown < 6 {
                println!("      {at:#05x}: built {a:02x}, real {b:02x}");
                shown += 1;
            }
        }
        let differing = built
            .bytes
            .iter()
            .zip(&real)
            .filter(|(a, b)| a != b)
            .count();
        println!("      {differing} of {} bytes differ", real.len());
    }

    if !all {
        return Err("a licence did not rebuild".into());
    }
    Ok(())
}
