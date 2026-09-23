//! Segment layout rules, and the linker script that encodes them.
//!
//! A module's layout is not a matter of taste. Three separate rules here were each learned by
//! producing a file a loader rejected or, worse, accepted and then read wrongly:
//!
//! - **Two loadable segments, not three.** A separate read-only segment is the obvious layout
//!   and one loader silently declines to map it, leaving every `.rodata` byte unmapped.
//! - **16 KiB alignment**, which is the console's allocation granularity and not the host's.
//! - **`.got` and `.got.plt` stay separate.** Merged, the linkage-table base names the wrong
//!   address and every resolved import lands at the wrong offset.
//!
//! `link/module.ld` at the repository root is the script, and its comments are the long form
//! of all three. This module exists so the constants it hard-codes can be checked: a linker
//! script is the one artefact here that no compiler validates, and a segment type that drifts
//! from [`crate::segment`] would produce a module whose headers are wrong in a way nothing in
//! the build notices.

/// The script, compiled in so the test below reads the real file.
pub const SCRIPT: &str = include_str!("../../../link/module.ld");

/// Linker script for native current-generation (Prospero) eboots.
pub const NATIVE_EBOOT_SCRIPT: &str = include_str!("../../../link/native_eboot.ld");

/// Linker script for older-generation (Orbis) eboots.
pub const EBOOT_SCRIPT: &str = include_str!("../../../link/eboot.ld");

/// The console's allocation granularity.
///
/// **Not the host's 4 KiB.** A segment that begins part-way into one of these cannot be
/// placed by a loader mapping whole pages, and the symptom is a fault inside the loader
/// rather than a rejection naming the cause.
pub const ALLOCATION_GRANULARITY: u64 = 0x4000;

/// How many loadable segments a module has.
///
/// Two: read-execute and read-write, with read-only data in the first. Adding a third for
/// `.rodata` is the obvious layout and it is wrong - one loader maps segments by kind and
/// simply omits it, so every byte of read-only data is absent at run time. Since the section
/// tables and every string live there, the module faults on its first act.
pub const LOADABLE_SEGMENTS: usize = 2;

/// Tags a module spends per imported library.
///
/// Four - the standard `NEEDED` plus three vendor tags - on top of the tags describing the
/// tables themselves. This is what actually drives how much room the dynamic table needs, and
/// getting it wrong is why the reservation in the script is sized by library count rather
/// than by a round number that looked large enough.
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

    #[test]
    fn the_script_declares_the_segment_types_this_crate_names() {
        // The one artefact here a compiler does not check. A segment type that drifts from
        // the constant produces a module whose headers are wrong and whose build succeeds.
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

    #[test]
    fn the_script_declares_exactly_two_loadable_segments() {
        // Three was the obvious layout and cost every `.rodata` byte at run time. Counted
        // rather than trusted, because the failure is silent in the build and total at run
        // time.
        let declared = SCRIPT
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !line.starts_with('*') && !line.starts_with("/*") && line.contains("PT_LOAD")
            })
            .count();
        assert_eq!(declared, LOADABLE_SEGMENTS);
    }

    #[test]
    fn the_got_and_the_linkage_table_stay_separate() {
        // Merging them pointed the linkage-table base at the start of `.got`, so every
        // resolved import landed at the wrong offset - a module that resolves nothing and
        // faults on its first call, which reads as the loader not supporting the format.
        assert!(SCRIPT.contains(".got            :"));
        assert!(SCRIPT.contains(".got.plt        :"));
    }

    #[test]
    fn the_headers_are_covered_by_the_first_segment() {
        // Without FILEHDR and PHDRS the header table is not inside any segment, and a loader
        // that maps only what the segments describe cannot read the table it just used.
        assert!(SCRIPT.contains("PT_LOAD FILEHDR PHDRS"));
    }

    #[test]
    fn eboot_scripts_declare_pt_tls_and_keep_bss_last_in_data() {
        for (name, script) in [
            ("native_eboot.ld", super::NATIVE_EBOOT_SCRIPT),
            ("eboot.ld", super::EBOOT_SCRIPT),
        ] {
            // Must declare PT_TLS segment (REQ-20260915T0001Z-8d72)
            assert!(
                script.contains("tls       PT_TLS FLAGS(4);"),
                "{name} should declare `tls PT_TLS FLAGS(4);`"
            );
            // Must place .tdata and .tbss in :data :tls
            assert!(
                script.contains(".tdata          : { *(.tdata .tdata.*) }   :data :tls"),
                "{name} should place .tdata in :data :tls"
            );
            assert!(
                script.contains(".tbss           : { *(.tbss .tbss.* .tcommon) } :data :tls"),
                "{name} should place .tbss in :data :tls"
            );

            // .bss MUST be placed after .dynamic so NOBITS is not forced to write zeros to the file (REQ-20260923T2350Z-6c1a)
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
