//! The relocations a module carries, decoded.
//!
//! A fixed-address executable should need very few. Where a value a loader reads *before*
//! relocating is supplied by one, the module states nothing in the file and the loader sees a
//! null - which is a different failure from the value being wrong.
//!
//! Two modes, because the interesting question changes with the size of the table:
//!
//!   relocs <file>...              the first few of each table, decoded
//!   relocs --summary <file>...    every entry, counted by type
//!   relocs --sym N <file>...      every entry naming symbol index N
//!
//! `--summary` exists because the listing is capped at sixteen and a module here carries
//! thirty-nine thousand: "all RELATIVE" was read off the first sixteen and was wrong about the
//! table as a whole. A count over the whole table cannot be read that way.
//!
//! `--sym` is for the opposite question - *this* import, where is its slot - which is what you
//! want when deciding whether an unresolved import can be repaired by writing to it.

fn unwrap(bytes: &[u8]) -> Vec<u8> {
    match selfish_container::Container::parse(bytes) {
        Ok(container) => container.to_elf().unwrap_or_else(|_| bytes.to_vec()),
        Err(_) => bytes.to_vec(),
    }
}

fn kind(raw: u32) -> &'static str {
    match raw {
        1 => "64",
        6 => "GLOB_DAT",
        7 => "JUMP_SLOT",
        8 => "RELATIVE",
        other => {
            let _ = other;
            "?"
        }
    }
}

fn main() {
    let mut summary = false;
    let mut want_sym: Option<u64> = None;
    let mut paths: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--summary" => summary = true,
            "--sym" => want_sym = args.next().and_then(|n| n.parse().ok()),
            other => paths.push(other.to_string()),
        }
    }

    for path in paths {
        let bytes = std::fs::read(&path).expect("a file");
        let inner = unwrap(&bytes);
        let elf = selfish_elf::Elf::parse(&inner).expect("an executable");
        let (blob, info) = elf.tables().expect("tables").expect("vendor tables");
        println!("== {path}");
        for (name, at, size) in [
            ("rela", info.rela, info.relasz),
            ("jmprel", info.jmprel, info.pltrelsz),
        ] {
            println!("   {name}: {} entries", size / 24);
            let mut shown = 0;
            // Indexed by the same numbers `kind` maps, plus a catch-all in the last slot so an
            // unrecognised type is counted rather than dropped - a table that silently omits
            // what it could not name reads as a table where that type does not occur.
            let mut counts = [0_u64; 10];
            let mut unknown = 0_u64;
            for n in 0..(size / 24) {
                let base = (at + n * 24) as usize;
                let Some(record) = blob.get(base..base + 24) else {
                    break;
                };
                let read = |o: usize| {
                    let mut raw = [0_u8; 8];
                    raw.copy_from_slice(&record[o..o + 8]);
                    u64::from_le_bytes(raw)
                };
                let (offset, info_word, addend) = (read(0), read(8), read(16));
                let ty = (info_word & 0xFFFF_FFFF) as u32;
                let sym = info_word >> 32;

                if summary {
                    match counts.get_mut(ty as usize) {
                        Some(slot) => *slot += 1,
                        None => unknown += 1,
                    }
                    continue;
                }
                if let Some(wanted) = want_sym {
                    if sym == wanted {
                        println!(
                            "     at {offset:#010x}  type {:<9} sym {sym:<4} addend {addend:#x}",
                            kind(ty)
                        );
                    }
                    continue;
                }
                if shown < 16 {
                    println!(
                        "     at {offset:#010x}  type {:<9} sym {sym:<4} addend {addend:#x}",
                        kind(ty)
                    );
                    shown += 1;
                }
            }
            if summary {
                for (ty, count) in counts.iter().enumerate() {
                    if *count > 0 {
                        println!("     {:<9} {count}", kind(ty as u32));
                    }
                }
                if unknown > 0 {
                    println!("     {:<9} {unknown}", "(other)");
                }
            }
        }
        println!();
    }
}
