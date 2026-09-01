//! Every container entry, decoded, beside the program header it describes.
//!
//! The auth manager loads a segment **block by block** and checks a size before it decrypts
//! anything. When it refuses - `_sceSblAuthMgrLoadSelfBlock: sz for b error` - nothing in the
//! dynamic table has been read yet, so the fault is in these four fields and not in anything
//! the segment contains.

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
fn flags(props: u64) -> String {
    let bit = |shift: u32| (props >> shift) & 1 == 1;
    let mut out = Vec::new();
    if bit(0) {
        out.push("ordered".to_owned());
    }
    if bit(1) {
        out.push("encrypted".to_owned());
    }
    if bit(2) {
        out.push("signed".to_owned());
    }
    if bit(3) {
        out.push("compressed".to_owned());
    }
    if bit(11) {
        // Encoded as ilog2(bytes) - 12, so 0x4000 stores as 2.
        let size = 1_u64 << (12 + ((props >> 12) & 0xF));
        out.push(format!("blocked({size:#x})"));
    }
    if bit(16) {
        out.push("digests".to_owned());
    }
    if bit(17) {
        out.push("extents".to_owned());
    }
    out.join(" ")
}

fn main() {
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).expect("a file");
        let Ok(container) = selfish_container::Container::parse(&bytes) else {
            println!("== {path}\n   not a container\n");
            continue;
        };
        println!(
            "== {path}  ({} bytes, {})",
            bytes.len(),
            container.generation()
        );
        // What each entry claims, and - for a blocked segment - whether the block arithmetic
        // comes out whole. A partial trailing block is normal; a partial *first* one is not.
        for (n, entry) in container.entries().iter().enumerate() {
            let blocked = (entry.props >> 11) & 1 == 1;
            let block = 1_u64 << (12 + ((entry.props >> 12) & 0xF));
            let blocks = if blocked && block > 0 {
                format!("{} block(s) of {block:#x}", entry.memsz.div_ceil(block))
            } else {
                "-".to_owned()
            };
            println!(
                "  [{n:2}] props {:#014x} seg {:>2} off {:#010x} filesz {:#010x} memsz {:#010x}",
                entry.props,
                entry.segment_index(),
                entry.offset,
                entry.filesz,
                entry.memsz,
            );
            println!(
                "       {} | {} | data {}",
                flags(entry.props),
                blocks,
                if entry.carries_segment_data() {
                    "yes"
                } else {
                    "no"
                }
            );
        }
        // The program headers, so a segment index means something.
        if let Ok(inner) = container.to_elf() {
            if let Ok(elf) = selfish_elf::Elf::parse(&inner) {
                println!(
                    "   e_type {:#06x} ({:?}), entry {:#x}",
                    elf.object_type().to_raw(),
                    elf.object_type(),
                    elf.entry()
                );
                println!("   program headers:");
                for (n, header) in elf.program_headers().iter().enumerate() {
                    println!(
                        "     [{n:2}] type {:#010x} off {:#010x} vaddr {:#010x} filesz {:#010x} memsz {:#010x} align {:#x}",
                        header.p_type.get(),
                        header.offset.get(),
                        header.vaddr.get(),
                        header.filesz.get(),
                        header.memsz.get(),
                        header.align.get(),
                    );
                }
            }
        }
        println!();
    }
}
