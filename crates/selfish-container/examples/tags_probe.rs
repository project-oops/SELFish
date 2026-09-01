//! The dynamic tags a module declares, ours beside a real one's.
//!
//! This crate reads back what it writes, and `dynamic` and `dynlib` agree with each other by
//! construction - so a round trip proves the pair is self-consistent and says nothing about
//! whether either matches a console's expectations. The tags in a real eboot are the only third
//! opinion available, and they have never been compared.
//!
//! It matters because a console's loader refuses this project's eboot while loading its dynamic
//! tables - `allocate_per_file_info_compact: Failed to load SCE_DYNLIBDATA: 5` - and the segment
//! header, the container entry and the segment's size have all been cleared already. What is left
//! is the shape of the tables inside it.
//!
//!     tags_probe <ours> <real>
// A diagnostic probe, held to a probe's standards rather than the library's. Indexing and
// arithmetic over offsets the format fixes is the clearest way to say what is being read, and
// nothing here ships: a wrong offset produces a wrong line on a terminal.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::format_collect,
    clippy::uninlined_format_args,
    clippy::unreadable_literal,
    clippy::doc_markdown,
    clippy::missing_panics_doc,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_lines
)]
use std::collections::BTreeMap;

/// A tag's name, so a diff reads as meanings rather than numbers.
fn name(tag: u64) -> String {
    use selfish_elf::dynamic::{standard, vendor};
    let known: &[(u64, &str)] = &[
        (standard::NEEDED, "NEEDED"),
        (standard::PLTRELSZ, "PLTRELSZ"),
        (standard::PLTGOT, "PLTGOT"),
        (standard::HASH, "HASH"),
        (standard::STRTAB, "STRTAB"),
        (standard::SYMTAB, "SYMTAB"),
        (standard::RELA, "RELA"),
        (standard::RELASZ, "RELASZ"),
        (standard::RELAENT, "RELAENT"),
        (standard::STRSZ, "STRSZ"),
        (standard::SYMENT, "SYMENT"),
        (standard::INIT, "INIT"),
        (vendor::MODULE_INFO, "SCE_MODULE_INFO"),
        (vendor::NEEDED_MODULE_LEGACY, "SCE_NEEDED_MODULE(legacy)"),
        (vendor::MODULE_ATTR_LEGACY, "SCE_MODULE_ATTR(legacy)"),
        (vendor::EXPORT_LIB_LEGACY, "SCE_EXPORT_LIB(legacy)"),
        (vendor::IMPORT_LIB_LEGACY, "SCE_IMPORT_LIB(legacy)"),
        (vendor::MODULE_INFO_CURRENT, "SCE_MODULE_INFO(current)"),
        (vendor::NEEDED_MODULE_CURRENT, "SCE_NEEDED_MODULE(current)"),
        (vendor::MODULE_ATTR_CURRENT, "SCE_MODULE_ATTR(current)"),
        (vendor::IMPORT_LIB_CURRENT, "SCE_IMPORT_LIB(current)"),
        (vendor::EXPORT_LIB_CURRENT, "SCE_EXPORT_LIB(current)"),
        (vendor::EXPORT_LIB_ATTR, "SCE_EXPORT_LIB_ATTR"),
        (vendor::IMPORT_LIB_ATTR, "SCE_IMPORT_LIB_ATTR"),
        (vendor::HASH, "SCE_HASH"),
        (vendor::PLTGOT, "SCE_PLTGOT"),
        (vendor::JMPREL, "SCE_JMPREL"),
        (vendor::PLTREL, "SCE_PLTREL"),
        (vendor::PLTRELSZ, "SCE_PLTRELSZ"),
        (vendor::RELA, "SCE_RELA"),
        (vendor::RELASZ, "SCE_RELASZ"),
        (vendor::RELAENT, "SCE_RELAENT"),
        (vendor::STRTAB, "SCE_STRTAB"),
        (vendor::STRSZ, "SCE_STRSZ"),
        (vendor::SYMTAB, "SCE_SYMTAB"),
        (vendor::SYMENT, "SCE_SYMENT"),
        (vendor::HASHSZ, "SCE_HASHSZ"),
        (vendor::SYMTABSZ, "SCE_SYMTABSZ"),
    ];
    for (value, label) in known {
        if *value == tag {
            return (*label).to_owned();
        }
    }
    format!("{tag:#x}")
}

fn tags(path: &str) -> Vec<(u64, u64)> {
    let bytes = std::fs::read(path).expect("a file");
    // A container stores its segments at its own offsets, so an eboot has to be reassembled
    // before the ELF offsets in its headers mean anything. A bare ELF passes through.
    let inner = match selfish_container::Container::parse(&bytes) {
        Ok(container) => container.to_elf().unwrap_or_else(|_| bytes.clone()),
        Err(_) => bytes.clone(),
    };
    let elf = selfish_elf::Elf::parse(&inner).expect("an executable");
    elf.dynamic_entries().expect("a dynamic table")
}

fn main() {
    let ours = std::env::args()
        .nth(1)
        .expect("usage: tags_probe <ours> <real>");
    // The second file is optional: with one argument this lists what that file emits, which
    // is the question you have when a loader is ignoring a declaration and you want to see
    // the declaration. With two it diffs them, which is the question you have when you have a
    // working module to compare against - and a working module is not always to hand.
    let real = std::env::args().nth(2);

    // How many times each tag appears, which is the comparison that matters: a tag a loader
    // requires and this crate never emits shows up as present-there, absent-here.
    let count = |list: &[(u64, u64)]| {
        let mut seen: BTreeMap<u64, usize> = BTreeMap::new();
        for (tag, _) in list {
            *seen.entry(*tag).or_insert(0) += 1;
        }
        seen
    };
    let o = tags(&ours);
    let r = real.as_deref().map(tags).unwrap_or_default();
    let (oc, rc) = (count(&o), count(&r));
    match &real {
        Some(_) => println!("ours: {} entries    real: {} entries\n", o.len(), r.len()),
        None => println!("{ours}: {} entries\n", o.len()),
    }

    let mut every: Vec<u64> = oc.keys().chain(rc.keys()).copied().collect();
    every.sort_unstable();
    every.dedup();

    println!("{:<24} {:>6} {:>6}", "tag", "ours", "real");
    for tag in every {
        let (a, b) = (
            oc.get(&tag).copied().unwrap_or(0),
            rc.get(&tag).copied().unwrap_or(0),
        );
        let mark = match (a, b) {
            (0, _) => "   <<< real has it, we never emit it",
            (_, 0) => "   <<< we emit it, real does not",
            _ => "",
        };
        println!("{:<24} {a:>6} {b:>6}{mark}", name(tag));
    }
}
