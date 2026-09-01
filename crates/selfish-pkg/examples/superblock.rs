//! How much of a filesystem superblock would a writer have to invent?
//!
//! The reader here knows five fields of a 0x400-byte superblock, because five is what a reader
//! needs. A *writer* has to produce all 0x400. This run says how much of that is actually
//! occupied in real images - the difference between "the rest is zero and writing one is
//! tractable" and "the rest is structure nobody here has a citable meaning for".
//!
//! It answers a question, it does not derive a format. Nothing here is written back into
//! `data/`; the point is to size a gap honestly. (principle 2)
//!
//! ```text
//! cargo run -p selfish-pkg --example superblock -- <package>...
//! ```

#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "an example reads better with plain arithmetic; the library it drives does not"
)]

/// The fields this repository has a cited meaning for, as `(name, offset, length)`.
const KNOWN: &[(&str, usize, usize)] = &[
    ("mode", 0x1C, 2),
    ("block_size", 0x20, 4),
    ("inode_count", 0x30, 8),
    ("inode_blocks", 0x40, 8),
    ("seed", 0x370, 16),
];

const SUPERBLOCK: usize = 0x400;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: superblock <package>...");
        std::process::exit(2);
    }

    for path in &paths {
        let bytes = std::fs::read(path)?;
        let package = selfish_pkg::Package::parse(&bytes)?;
        let at = usize::try_from(package.image_offset()?)?;
        let superblock = bytes
            .get(at..at + SUPERBLOCK)
            .ok_or("the image runs past the end of the package")?;

        // Which bytes are accounted for by a field we can name.
        let mut accounted = vec![false; SUPERBLOCK];
        for (_, offset, len) in KNOWN {
            for byte in accounted.iter_mut().skip(*offset).take(*len) {
                *byte = true;
            }
        }

        let occupied: Vec<usize> = (0..SUPERBLOCK)
            .filter(|at| superblock[*at] != 0 && !accounted[*at])
            .collect();

        println!("{}", path.rsplit(['/', '\\']).next().unwrap_or(path));
        println!(
            "  {} of {SUPERBLOCK} bytes are named by a cited field",
            KNOWN.iter().map(|(_, _, len)| len).sum::<usize>()
        );
        println!(
            "  {} further bytes are non-zero and unaccounted for",
            occupied.len()
        );

        // Runs, because a run is a field and scattered bytes are not.
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for at in occupied {
            match runs.last_mut() {
                Some((start, len)) if *start + *len == at => *len += 1,
                _ => runs.push((at, 1)),
            }
        }
        for (start, len) in &runs {
            println!("    {start:#06x}..{:#06x}  {len} bytes", start + len);
        }
        if runs.is_empty() {
            println!("    (none - the rest of the superblock is zero)");
        }
    }
    Ok(())
}
