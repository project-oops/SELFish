//! The process parameter block, and whatever its pointers point at.
//!
//! `libkernel` reads this before a single instruction of the module runs, so a field it
//! expects and does not find faults inside the platform library on a stack frame that names
//! nothing of ours. Printing the block alone is not enough - the interesting part is what the
//! non-null entries lead to, because that is what says whether a slot holds a string, a
//! counted structure, or another table.

fn unwrap(bytes: &[u8]) -> Vec<u8> {
    match selfish_container::Container::parse(bytes) {
        Ok(container) => container.to_elf().unwrap_or_else(|_| bytes.to_vec()),
        Err(_) => bytes.to_vec(),
    }
}

/// Turn a virtual address into a file offset, using whichever segment maps it.
fn at_address(elf: &selfish_elf::Elf<'_>, address: u64) -> Option<u64> {
    elf.program_headers()
        .iter()
        .filter(|h| h.vaddr.get() != 0 && h.filesz.get() != 0)
        .find(|h| address >= h.vaddr.get() && address < h.vaddr.get() + h.filesz.get())
        .map(|h| h.offset.get() + (address - h.vaddr.get()))
}

fn dump(bytes: &[u8], at: usize, len: usize, indent: &str) {
    let end = at.saturating_add(len).min(bytes.len());
    let Some(slice) = bytes.get(at..end) else {
        return;
    };
    for (n, row) in slice.chunks(16).enumerate() {
        let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
        let text: String = row
            .iter()
            .map(|b| {
                if b.is_ascii_graphic() || *b == b' ' {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("{indent}+{:#05x}  {:<47}  {text}", n * 16, hex.join(" "));
    }
}

fn main() {
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).expect("a file");
        let inner = unwrap(&bytes);
        let elf = selfish_elf::Elf::parse(&inner).expect("an executable");
        let Some(header) = elf
            .program_headers()
            .iter()
            .find(|h| h.p_type.get() == selfish_elf::segment::SCE_PROCPARAM)
        else {
            println!("== {path}\n   no process param\n");
            continue;
        };
        let at = header.offset.get() as usize;
        let size = header.filesz.get() as usize;
        let block = inner.get(at..at.saturating_add(size)).unwrap_or(&[]);
        println!("== {path}");
        println!(
            "   {size:#x} bytes at {at:#x}, vaddr {:#x}",
            header.vaddr.get()
        );
        dump(&inner, at, size, "   ");

        // Every eight-byte slot past the header, followed where it leads.
        println!("   slots:");
        for (n, word) in block.chunks(8).enumerate().skip(3) {
            let mut raw = [0_u8; 8];
            raw[..word.len()].copy_from_slice(word);
            let value = u64::from_le_bytes(raw);
            if value == 0 {
                println!("     +{:#04x}  null", n * 8);
                continue;
            }
            let Some(target) = at_address(&elf, value) else {
                println!("     +{:#04x}  {value:#x}  (maps to no segment)", n * 8);
                continue;
            };
            println!("     +{:#04x}  {value:#x} -> file {target:#x}", n * 8);
            dump(&inner, target as usize, 0x30, "            ");
        }
        println!();
    }
}
