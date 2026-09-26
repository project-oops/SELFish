//! Every defined export of a module with a nonzero address, one `name 0xvalue` line each and
//! nothing else on stdout. The name is the encoded `hash#library#module` an importer matches;
//! the value is the offset a loader adds to the module's load base.

// A probe reads fixed offsets and prints them, so the library's arithmetic lints do not apply.
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
