//! Every dynamic tag's *value*, ours beside a real one, with a bounds verdict.
//!
//! Counting tags proves only that a field exists. The failure this exists to chase is a
//! field that exists and points somewhere wrong - the vendor table mixes offsets into the
//! dynlib blob with virtual addresses, and a writer that picks the wrong one produces a file
//! that parses perfectly and refuses to load.

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

fn report(path: &str) {
    let bytes = std::fs::read(path).expect("a file");
    let inner = unwrap(&bytes);
    let elf = selfish_elf::Elf::parse(&inner).expect("an executable");
    let entries = elf.dynamic_entries().expect("a dynamic table");
    let tables = elf.tables().ok().flatten();
    let blob_len = tables.as_ref().map_or(0, |(blob, _)| blob.len());

    println!("== {path}");
    println!("   dynlib blob: {blob_len} bytes, {} tags", entries.len());
    if let Some((blob, info)) = &tables {
        // The head of the blob, because whatever sits before the string table is a region
        // no tag we write accounts for.
        let head = &blob[..blob.len().min(0x20)];
        print!("   head:");
        for (n, byte) in head.iter().enumerate() {
            if n % 16 == 0 && n > 0 {
                print!("\n        ");
            }
            print!(" {byte:02x}");
        }
        println!();
        // Where each table claims to be. "inside" means the value works as an offset into
        // the blob; a value far past the end is an address and must be rebased instead.
        let verdict = |name: &str, at: u64, size: u64| {
            let end = at.saturating_add(size);
            let inside = blob_len as u64 >= end && size > 0;
            println!(
                "   {name:<10} at {at:#010x} size {size:#010x}  {}",
                if inside {
                    "inside the blob"
                } else {
                    "size not stated"
                }
            );
        };
        verdict("strtab", info.strtab, info.strsz);
        verdict("symtab", info.symtab, info.symtabsz);
        verdict("hash", info.hash, 0);
        verdict("rela", info.rela, info.relasz);
        verdict("jmprel", info.jmprel, info.pltrelsz);
        println!(
            "   import libs {}, needed modules {}, DT_NEEDED {}",
            info.import_libs.len(),
            info.needed_modules.len(),
            info.needed.len()
        );
    }
    // Only the vendor tags outside the table run, plus anything standard - the run itself is
    // already summarised above and repeating 30 offsets buries the interesting rows.
    println!("   tags the summary does not cover:");
    for (tag, value) in &entries {
        let vendor_table_run =
            std::env::var("ALL_TAGS").is_err() && (0x6100_0025..=0x6100_003F).contains(tag);
        if vendor_table_run {
            continue;
        }
        println!("     {tag:#012x} = {value:#018x}");
    }
    println!();
}

fn main() {
    for path in std::env::args().skip(1) {
        report(&path);
    }
}
