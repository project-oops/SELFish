//! The encoded names of a module's imported symbols.
//!
//! An import's name carries its library and module ids in the two suffixes, so the symbol
//! table is a second, independent statement of the same numbering the vendor tags declare.
//! Where those two disagree, a loader believes one of them and the module does not load.

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
    clippy::collapsible_if,
    clippy::format_collect,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::doc_markdown,
    clippy::missing_panics_doc,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_lines
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
        .expect("usage: symbol_names <executable>");
    let limit: usize = std::env::args()
        .nth(2)
        .map_or(12, |n| n.parse().unwrap_or(12));
    let bytes = std::fs::read(&path).expect("a file");
    let inner = unwrap(&bytes);
    let elf = selfish_elf::Elf::parse(&inner).expect("an executable");
    let (blob, info) = elf.tables().expect("tables").expect("vendor tables");

    let at = info.strtab as usize;
    let end = at.saturating_add(info.strsz as usize).min(blob.len());
    let strings = blob.get(at..end).unwrap_or(&[]);

    println!("== {path}");
    let count = (info.symtabsz / info.syment.max(1)) as usize;
    println!("   {count} symbols");
    let mut suffixes: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut shown = 0;
    for n in 0..count {
        let base = info.symtab as usize + n * info.syment as usize;
        let Some(record) = blob.get(base..base + 24) else {
            break;
        };
        let name_at = u32::from_le_bytes([record[0], record[1], record[2], record[3]]) as usize;
        if name_at == 0 || name_at >= strings.len() {
            continue;
        }
        let stop = strings[name_at..]
            .iter()
            .position(|b| *b == 0)
            .map_or(strings.len(), |k| name_at + k);
        let name = String::from_utf8_lossy(&strings[name_at..stop]).into_owned();
        // The suffix is what carries the ids; the head is the hash and differs per symbol.
        if let Some(hash_end) = name.find('#') {
            *suffixes.entry(name[hash_end..].to_owned()).or_default() += 1;
        }
        if shown < limit {
            println!("     {name}");
            shown += 1;
        }
    }
    println!("   id suffixes seen (suffix = #library#module):");
    for (suffix, n) in &suffixes {
        println!("     {suffix:<10} x{n}");
    }

    // The binding, type and section index of every entry, counted.
    //
    // The name is what a reader looks at, and it is not what a loader decides on. `st_info`
    // splits into a binding (GLOBAL, WEAK, LOCAL) and a type (FUNC, OBJECT, NOTYPE), and
    // `st_shndx` says whether the entry is defined here or expected from somewhere else. A
    // loader entitled to skip weak undefined symbols would leave exactly the imports this
    // project cannot get bound, and the name would look correct throughout.
    let mut shapes: std::collections::BTreeMap<(u8, u8, u16), usize> =
        std::collections::BTreeMap::new();
    for n in 0..count {
        let base = info.symtab as usize + n * info.syment as usize;
        let Some(record) = blob.get(base..base + 24) else {
            break;
        };
        let info_byte = record[4];
        let shndx = u16::from_le_bytes([record[6], record[7]]);
        *shapes
            .entry((info_byte >> 4, info_byte & 0x0f, shndx))
            .or_default() += 1;
    }
    let bind = |b: u8| match b {
        0 => "LOCAL",
        1 => "GLOBAL",
        2 => "WEAK",
        _ => "?",
    };
    let kind = |t: u8| match t {
        0 => "NOTYPE",
        1 => "OBJECT",
        2 => "FUNC",
        _ => "?",
    };
    println!("   symbol shapes (binding, type, section - 0 means undefined):");
    for ((b, t, shndx), n) in &shapes {
        println!("     {:<7} {:<7} shndx {shndx:<5} x{n}", bind(*b), kind(*t));
    }
}
