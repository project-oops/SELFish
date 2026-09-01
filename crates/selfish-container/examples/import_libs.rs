//! Each imported library, with the **version** the module declares for it.
//!
//! A loader builds its lookup key from this version and matches it against the version the
//! library was registered with. A mismatch does not fail: every symbol from that library
//! silently fails to resolve, the module loads, runs, and finds none of its imports. See
//! `data/library-versions.tsv` - the one library with an exception is the one that decides
//! whether anything appears on screen.

// A diagnostic probe, held to a probe's standards rather than the library's.
//
// These read structures whose layout is already known, at offsets the format fixes, and print
// what they find. Indexing, slicing and plain arithmetic over those offsets is the clearest way
// to say what is being read - a probe that wraps every field access in a fallible conversion is
// harder to check against a hex dump, which is the only thing it will ever be checked against.
// Nothing here ships: a wrong offset produces a wrong line on a terminal, not a wrong file.
//
// The library itself keeps every one of these lints. This block is the boundary between the two.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::expect_used,
    clippy::unwrap_used
)]

fn unwrap(bytes: &[u8]) -> Vec<u8> {
    match selfish_container::Container::parse(bytes) {
        Ok(container) => container.to_elf().unwrap_or_else(|_| bytes.to_vec()),
        Err(_) => bytes.to_vec(),
    }
}

fn main() {
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).expect("a file");
        let inner = unwrap(&bytes);
        let elf = selfish_elf::Elf::parse(&inner).expect("an executable");
        let (blob, info) = elf.tables().expect("tables").expect("vendor tables");
        let at = info.strtab as usize;
        let end = at.saturating_add(info.strsz as usize).min(blob.len());
        let strings = blob.get(at..end).unwrap_or(&[]);
        let name_at = |offset: u64| -> String {
            let offset = offset as usize;
            if offset >= strings.len() {
                return String::from("<out of range>");
            }
            let stop = strings[offset..]
                .iter()
                .position(|b| *b == 0)
                .map_or(strings.len(), |n| offset + n);
            String::from_utf8_lossy(&strings[offset..stop]).into_owned()
        };
        println!("== {path}");
        println!("   needed modules (id, module version, name):");
        for packed in &info.needed_modules {
            let id = packed >> 48;
            let major = (packed >> 40) & 0xFF;
            let minor = (packed >> 32) & 0xFF;
            let name = name_at(packed & 0xFFFF_FFFF);
            // The raw word beside the decode, because a decode that reads cleanly is not
            // evidence the word is well formed - every field this splits out is a guess about
            // where the boundaries are, and a loader that stops part-way through the table
            // will have stopped on something this prints as ordinary. (obscene#D241)
            println!("   id {id:>3}  module {major}.{minor}  {name:<26} raw {packed:#018x}");
        }
        println!("   import libraries (id, library version, name):");
        for packed in &info.import_libs {
            // id in the top sixteen bits, version in the middle, name offset in the low half.
            let id = packed >> 48;
            let version = (packed >> 32) & 0xFFFF;
            let name = name_at(packed & 0xFFFF_FFFF);
            println!("   id {id:>3}  version {version}  {name:<26} raw {packed:#018x}");
        }
        // The attribute word beside the identity word.
        //
        // Every library gets both, and only the identity word decodes into something a human
        // recognises - so a wrong attribute is invisible in a listing that shows names and ids
        // and looks entirely correct. That is exactly the shape of a module a loader maps and
        // then declines to bind. (obscene#D241)
        let entries = elf.dynamic_entries().unwrap_or_default();
        println!("   library attributes (id, attribute word):");
        for (tag, value) in &entries {
            if *tag == 0x6100_0019 {
                println!(
                    "   id {:>3}  attr {:#x}   raw {value:#018x}",
                    value >> 48,
                    value & 0xFFFF_FFFF
                );
            }
        }
        for (tag, value) in &entries {
            if *tag == 0x6100_0011 || *tag == 0x6100_0047 {
                println!("   module attr (tag {tag:#x})  raw {value:#018x}");
            }
        }
        println!();
    }
}
