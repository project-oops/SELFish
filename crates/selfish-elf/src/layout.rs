//! Segment layout rules, and the linker script that encodes them.
//!
//! A module has two loadable segments, 16 KiB alignment, and separate `.got` and `.got.plt`.
//! `link/module.ld` at the repository root is the script. No compiler validates a linker
//! script, so the tests here check its hard-coded constants against [`crate::segment`].

/// The script, compiled in so the test below reads the real file.
pub const SCRIPT: &str = include_str!("../../../link/module.ld");

/// Linker script for native current-generation (Prospero) eboots.
pub const NATIVE_EBOOT_SCRIPT: &str = include_str!("../../../link/native_eboot.ld");

/// Linker script for older-generation (Orbis) eboots.
pub const EBOOT_SCRIPT: &str = include_str!("../../../link/eboot.ld");

/// The hardware's allocation granularity, not the host's 4 KiB page.
///
/// A segment that begins part-way into one faults inside the loader rather than being
/// rejected.
pub const ALLOCATION_GRANULARITY: u64 = 0x4000;

/// How many loadable segments a module has.
///
/// Read-execute and read-write, with read-only data in the first. A loader maps segments by
/// kind and omits a separate read-only one, leaving `.rodata` unmapped.
pub const LOADABLE_SEGMENTS: usize = 2;

/// Tags a module spends per imported library.
///
/// The standard `NEEDED` plus three vendor tags. The script sizes its dynamic-table
/// reservation by library count from this.
pub const TAGS_PER_LIBRARY: usize = 4;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use super::{ALLOCATION_GRANULARITY, LOADABLE_SEGMENTS, SCRIPT};
    use crate::segment;

    /// The script's vendor segment types match the constants in `segment`.
    #[test]
    fn the_script_declares_the_segment_types_this_crate_names() {
        for (name, value) in [
            ("dynlibdata", segment::SCE_DYNLIBDATA),
            ("procparam", segment::SCE_PROCPARAM),
        ] {
            let declaration = format!("{name} {value:#010x}").to_lowercase();
            assert!(
                SCRIPT.to_lowercase().contains(&declaration),
                "the script should declare `{declaration}`"
            );
        }
    }

    /// The script aligns to the hardware's granularity, never the host page size.
    #[test]
    fn the_script_aligns_to_the_consoles_granularity_and_not_the_hosts() {
        let granularity = format!("ALIGN({ALLOCATION_GRANULARITY:#x})");
        assert!(
            SCRIPT.contains(&granularity),
            "the script should align to {granularity}"
        );
        assert!(
            !SCRIPT.contains("ALIGN(0x1000)"),
            "0x1000 is the host's page size and placing a segment on it faults inside the \
             loader rather than being rejected"
        );
    }

    /// The script declares exactly `LOADABLE_SEGMENTS` `PT_LOAD` segments.
    #[test]
    fn the_script_declares_exactly_two_loadable_segments() {
        let declared = SCRIPT
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !line.starts_with('*') && !line.starts_with("/*") && line.contains("PT_LOAD")
            })
            .count();
        assert_eq!(declared, LOADABLE_SEGMENTS);
    }

    /// `.got` and `.got.plt` are separate output sections, so the linkage-table base is right.
    #[test]
    fn the_got_and_the_linkage_table_stay_separate() {
        assert!(SCRIPT.contains(".got            :"));
        assert!(SCRIPT.contains(".got.plt        :"));
    }

    /// The first segment maps the file and program headers.
    #[test]
    fn the_headers_are_covered_by_the_first_segment() {
        assert!(SCRIPT.contains("PT_LOAD FILEHDR PHDRS"));
    }

    /// Both eboot scripts declare `PT_TLS`, place TLS in it, and put `.bss` after `.dynamic`.
    #[test]
    fn eboot_scripts_declare_pt_tls_and_keep_bss_last_in_data() {
        for (name, script) in [
            ("native_eboot.ld", super::NATIVE_EBOOT_SCRIPT),
            ("eboot.ld", super::EBOOT_SCRIPT),
        ] {
            assert!(
                script.contains("tls       PT_TLS FLAGS(4);"),
                "{name} should declare `tls PT_TLS FLAGS(4);`"
            );
            assert!(
                script.contains(".tdata          : { *(.tdata .tdata.*) }   :data :tls"),
                "{name} should place .tdata in :data :tls"
            );
            assert!(
                script.contains(".tbss           : { *(.tbss .tbss.* .tcommon) } :data :tls"),
                "{name} should place .tbss in :data :tls"
            );

            // `.bss` after `.dynamic` keeps NOBITS from being written to the file as zeros.
            let bss_pos = script
                .find(".bss            :")
                .unwrap_or_else(|| panic!("{name} missing .bss section"));
            let dynamic_pos = script
                .find(".dynamic        :")
                .unwrap_or_else(|| panic!("{name} missing .dynamic section"));
            assert!(
                bss_pos > dynamic_pos,
                "{name}: .bss must be placed after .dynamic in the :data segment"
            );
        }
    }
}
