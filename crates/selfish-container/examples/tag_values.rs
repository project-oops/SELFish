//! What a real executable puts in the tags we never emit.
//!
//! `tags_probe` counts tags; this reads their *values*. A vendor tag whose value is a small
//! number is almost always an offset into the string table, and the string there names what
//! the tag is for - which is the only way to tell a filename from a fingerprint without a
//! vendor header.

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
        .expect("usage: tag_values <executable>");
    let bytes = std::fs::read(&path).expect("a file");
    let inner = unwrap(&bytes);
    let elf = selfish_elf::Elf::parse(&inner).expect("an executable");
    let entries = elf.dynamic_entries().expect("a dynamic table");

    // `tables` hands back the vendor data blob and the offsets into it that the tags gave.
    let tables = elf.tables().ok().flatten();
    let strings: &[u8] = match &tables {
        Some((blob, info)) => {
            let at = info.strtab as usize;
            let end = at.saturating_add(info.strsz as usize).min(blob.len());
            blob.get(at..end).unwrap_or(&[])
        }
        None => &[],
    };
    let read_string = |at: u64| -> String {
        let at = at as usize;
        if at >= strings.len() {
            return format!("<past the {}-byte string table>", strings.len());
        }
        let end = strings[at..]
            .iter()
            .position(|b| *b == 0)
            .map_or(strings.len(), |n| at + n);
        String::from_utf8_lossy(&strings[at..end]).into_owned()
    };

    println!("{path}");
    println!("  string table: {} bytes", strings.len());
    // The first few strings, because a module's own name is conventionally near the front.
    let mut at = 0usize;
    let mut shown = 0;
    while at < strings.len() && shown < 6 {
        let end = strings[at..]
            .iter()
            .position(|b| *b == 0)
            .map_or(strings.len(), |n| at + n);
        if end > at {
            println!(
                "    +{at:#06x} {:?}",
                String::from_utf8_lossy(&strings[at..end])
            );
            shown += 1;
        }
        at = end + 1;
    }
    println!();
    for (tag, value) in &entries {
        // Only the vendor range, and only the ones outside the run selfish already writes.
        if *tag < 0x6100_0000 || *tag > 0x6100_00FF {
            continue;
        }
        let low = tag & 0xFF;
        if !(low == 0x07 || low == 0x09 || low == 0x0B || low == 0x0D) {
            continue;
        }
        println!("  {tag:#x}  value {value:#018x}");
        println!(
            "            whole value as an offset: {:?}",
            read_string(*value)
        );
        // Vendor module-info values pack a name offset in the low half and a version above it.
        println!(
            "            low 32 as an offset:      {:?}",
            read_string(value & 0xFFFF_FFFF)
        );
    }
}
