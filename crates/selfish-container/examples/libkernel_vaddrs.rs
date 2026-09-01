//! Every defined export of a vendor module, as `name 0xvalue` - one line each.
//!
//! A loader resolving a title's imports against this library needs each export's address, and
//! `dynamic::symbols` already carries it. Two fields decide what to print: `section` is zero for
//! an import and nonzero for a symbol this module *defines* (the same test `Symbol::is_import`
//! makes), and `value` is that definition's offset within the module - the number a loader adds
//! to the module's load base to reach the function. An export with a zero value is a definition
//! with no address to resolve to, so both must be nonzero.
//!
//! The `name` is the entry's string. For a vendor module that string is the NID-encoded
//! identifier - `hash#library#module` - which is exactly what an importer matches on, so the two
//! sides of a resolution meet on it.
//!
//! Output is data only: nothing but `name 0xvalue` lines reaches stdout, so a consumer can
//! redirect it straight into a file. It is a dump of one real file and cites that file; it does
//! not decide a format, which is why it lives in an example rather than in the library.

// A diagnostic probe, held to a probe's standards rather than the library's - the same boundary
// symbol_names.rs draws. These read a layout the format already fixes, at offsets it fixes, and
// print what they find; a probe that wrapped every field access in a fallible conversion would be
// harder to check against a hex dump, which is the only thing it is ever checked against. Nothing
// here ships: a wrong offset produces a wrong line on a terminal, not a wrong file.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::doc_markdown,
    clippy::missing_panics_doc,
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
    let path = std::env::args()
        .nth(1)
        .expect("usage: libkernel_vaddrs <module.sprx>");
    let bytes = std::fs::read(&path).expect("a file");
    let inner = unwrap(&bytes);
    let elf = selfish_elf::Elf::parse(&inner).expect("an executable");
    let (blob, info) = elf.tables().expect("tables").expect("vendor tables");

    // The library's own reader, rather than a second hand-rolled walk of the same table.
    let symbols = selfish_elf::dynamic::symbols(blob, &info).expect("a symbol table");

    let at = info.strtab as usize;
    let end = at.saturating_add(info.strsz as usize).min(blob.len());
    let strings = blob.get(at..end).unwrap_or(&[]);

    for symbol in symbols {
        // A defined export with an address: section nonzero, value nonzero.
        if symbol.section == 0 || symbol.value == 0 {
            continue;
        }
        let name_at = symbol.name_offset as usize;
        if name_at >= strings.len() {
            continue;
        }
        let stop = strings[name_at..]
            .iter()
            .position(|b| *b == 0)
            .map_or(strings.len(), |k| name_at + k);
        let name = String::from_utf8_lossy(&strings[name_at..stop]);
        println!("{name} 0x{:x}", symbol.value);
    }
}
