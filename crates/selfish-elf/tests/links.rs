//! The linker script actually links, and produces the layout it claims to.
//!
//! `link/module.ld` is the one artefact in this repository that no compiler checks. Its unit
//! tests assert that its text names the right constants; this asserts that a linker fed it
//! emits the segments those constants describe, and then reads the result back through this
//! crate's own parser.
//!
//! # Skipped rather than failed when there is no toolchain
//!
//! `clang` and `ld.lld` are not build dependencies of this repository and will not be present
//! everywhere. A test that fails on a machine without them teaches people to ignore failures,
//! which costs more than the coverage is worth. It prints what it skipped and why.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::too_many_lines,
    reason = "a panic in a test is the test failing"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// A freestanding object with one section per thing the script has to place.
const SOURCE: &str = r#"
__attribute__((section(".sce_process_param"))) const unsigned long param[4] = {0};
__attribute__((section(".sce_dynlibdata"))) const unsigned char dynlib[1] = {0};
__attribute__((section(".interp"))) const char interp[] = "/system/common/lib/libkernel.sprx";
const char message[] = "in rodata";
int value = 7;
int uninitialised;
int _start(void) { return value + uninitialised + (int)message[0]; }
"#;

fn available(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../link/module.ld")
}

#[test]
fn the_script_produces_the_layout_it_describes() {
    if !available("clang") || !available("ld.lld") {
        println!("skipped: needs clang and ld.lld, which are not build dependencies here");
        return;
    }

    let dir = std::env::temp_dir().join("selfish-link-test");
    std::fs::create_dir_all(&dir).expect("a working directory");
    let source = dir.join("module.c");
    let object = dir.join("module.o");
    let linked = dir.join("module.elf");
    std::fs::write(&source, SOURCE).expect("writing the source");

    let compile = Command::new("clang")
        .args(["--target=x86_64-unknown-none", "-ffreestanding"])
        .args(["-fno-stack-protector", "-fPIC", "-c"])
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .output()
        .expect("running clang");
    assert!(
        compile.status.success(),
        "clang failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let link = Command::new("ld.lld")
        .arg("-T")
        .arg(script())
        .args(["--shared", "--no-rosegment"])
        .arg("-o")
        .arg(&linked)
        .arg(&object)
        .output()
        .expect("running ld.lld");
    assert!(
        link.status.success(),
        "the script did not link: {}",
        String::from_utf8_lossy(&link.stderr)
    );

    let bytes = std::fs::read(&linked).expect("the linked module");
    let elf = selfish_elf::Elf::parse(&bytes).expect("a readable module");
    let types: Vec<u32> = elf
        .program_headers()
        .iter()
        .map(|header| header.p_type.get())
        .collect();

    // Two loadable segments and not three. Three is the obvious layout and one loader
    // silently declines to map the extra one, leaving every byte of read-only data absent.
    assert_eq!(
        types
            .iter()
            .filter(|kind| **kind == selfish_elf::segment::LOAD)
            .count(),
        selfish_elf::layout::LOADABLE_SEGMENTS,
        "segment types were {types:#x?}"
    );

    for required in [
        selfish_elf::segment::DYNAMIC,
        selfish_elf::segment::INTERP,
        selfish_elf::segment::SCE_PROCPARAM,
        selfish_elf::segment::SCE_DYNLIBDATA,
    ] {
        assert!(
            types.contains(&required),
            "no {required:#x} segment; the script emitted {types:#x?}"
        );
    }

    // The headers have to be inside the first mapping. A loader that maps only what the
    // segments describe cannot otherwise read the header table it just used.
    let first = elf
        .program_headers()
        .iter()
        .find(|header| header.p_type.get() == selfish_elf::segment::LOAD)
        .expect("a loadable segment");
    assert_eq!(
        first.offset.get(),
        0,
        "the first loadable segment should start at the file header"
    );
}
