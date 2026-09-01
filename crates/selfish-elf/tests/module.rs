//! The whole chain, once: linker script → linker → vendor segment → back through the reader.
//!
//! Each piece has its own tests against synthetic input. This is the one that says they agree
//! about a real linked file, which is where the disagreements have historically been.
//!
//! Skipped rather than failed without `clang` and `ld.lld`; see `links.rs` for why.

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

use selfish_elf::dynamic::{self, Table};
use selfish_elf::dynlib::{self, Library, Linked};

/// A module that imports one platform function and defines an initialiser.
const SOURCE: &str = r#"
__attribute__((section(".sce_process_param"))) const unsigned long param[4] = {0};
__attribute__((section(".sce_dynlibdata"))) const unsigned char dynlib[1] = {0};
__attribute__((section(".interp"))) const char interp[] = "/system/common/lib/libkernel.sprx";

extern int sceKernelLoadStartModule(const char *name);
const char module_name[] = "probe.prx";

int probe_module_init(void) { return 0; }
int _start(void) { return sceKernelLoadStartModule(module_name); }
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

fn link(dir: &Path) -> Option<Vec<u8>> {
    if !available("clang") || !available("ld.lld") {
        println!("skipped: needs clang and ld.lld, which are not build dependencies here");
        return None;
    }
    std::fs::create_dir_all(dir).expect("a working directory");
    let source = dir.join("probe.c");
    let object = dir.join("probe.o");
    let linked = dir.join("probe.elf");
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

    let result = Command::new("ld.lld")
        .arg("-T")
        .arg(script())
        .args([
            "--shared",
            "--no-rosegment",
            "--unresolved-symbols=ignore-all",
        ])
        .arg("-o")
        .arg(&linked)
        .arg(&object)
        .output()
        .expect("running ld.lld");
    assert!(
        result.status.success(),
        "the script did not link: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Some(std::fs::read(&linked).expect("the linked module"))
}

#[test]
fn a_linked_module_becomes_one_the_reader_understands() {
    let dir = std::env::temp_dir().join("selfish-module-test");
    let Some(mut bytes) = link(&dir) else {
        return;
    };

    // Pull the linked tables out of their sections. This is the half `section.rs` exists for.
    let (symbols, names, jmprel, rela, pltgot, init) = {
        let elf = selfish_elf::Elf::parse(&bytes).expect("a readable module");
        let sections = elf
            .sections()
            .expect("a readable section table")
            .expect("a linked object has sections");

        let dynsym = sections.find(".dynsym").expect(".dynsym");
        let strings = sections
            .headers()
            .get(dynsym.link.get() as usize)
            .and_then(|header| sections.contents(header))
            .expect("its string table");

        let init = sections.symbols().and_then(|(symbols, strings)| {
            symbols
                .iter()
                .find(|symbol| {
                    !symbol.is_undefined()
                        && selfish_elf::section::string_at(strings, symbol.name_offset)
                            == Some("probe_module_init")
                })
                .map(|symbol| symbol.value)
        });

        (
            sections.contents(dynsym).expect(".dynsym bytes").to_vec(),
            strings.to_vec(),
            sections
                .find(".rela.plt")
                .and_then(|header| sections.contents(header))
                .unwrap_or_default()
                .to_vec(),
            sections
                .find(".rela.dyn")
                .and_then(|header| sections.contents(header))
                .unwrap_or_default()
                .to_vec(),
            sections
                .find(".got.plt")
                .or_else(|| sections.find(".got"))
                .map_or(0, |header| header.addr.get()),
            init,
        )
    };

    assert!(init.is_some(), "the module defines an initialiser");

    let libraries = vec![Library {
        name: "libkernel".to_owned(),
        id: 0,
        module_id: 0,
    }];
    let segment = dynlib::build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &jmprel,
            rela: &rela,
            pltgot,
        },
        "probe",
        &libraries,
        &|name| {
            // The manifest's job: which library, and what identifier. Hashed here because
            // this import has a name; one that arrived already encoded would `Nid::decode`.
            (name == "sceKernelLoadStartModule").then(|| dynlib::Resolution {
                nid: selfish_nid::Nid::with_suffix(name, &selfish_nid::suffix()),
                library: 0,
                module: 0,
            })
        },
    )
    .expect("a vendor segment");
    assert_eq!(segment.encoded, 1, "one import was re-encoded");

    let installed = dynlib::install(
        &mut bytes,
        &segment,
        Table::Legacy,
        selfish_abi::Generation::Previous,
        init,
    )
    .expect("an installed module");
    assert!(installed.tags > 0);
    assert_eq!(
        installed.table_base, 0,
        "the legacy convention leaves the tables unmapped"
    );

    // The identity a loader checks before it reads anything else. A linker leaves `e_type` as
    // an ordinary shared object and `EI_OSABI` as SysV, and a loader refuses both outright.
    let stamped = selfish_elf::identity::stamp(
        &mut bytes,
        selfish_elf::ObjectType::Executable,
        selfish_abi::Generation::Previous,
    )
    .expect("a stamped module");
    assert!(
        stamped.iter().any(|change| change.field == "e_type"),
        "the linker leaves an ordinary shared object, which a loader refuses: {stamped:?}"
    );

    // And now read the result as a loader would, through code that knows nothing about how it
    // was made.
    let elf = selfish_elf::Elf::parse(&bytes).expect("the rebuilt module");
    assert_eq!(elf.object_type(), selfish_elf::ObjectType::Executable);
    assert!(elf.has_platform_osabi());
    assert!(
        elf.sections().expect("no error").is_none(),
        "the section headers are gone, so there is exactly one description of every table"
    );

    let entries = elf.dynamic_entries().expect("a dynamic table");
    let info = dynamic::Info::from_entries(&entries);
    assert_eq!(info.table, Some(Table::Legacy));

    let vendor = elf.vendor_segment().expect("the appended segment");
    let imports = dynamic::imports(vendor, &info).expect("imports");
    assert_eq!(imports.len(), 1, "the one import survives the round trip");
    assert_eq!(imports[0].library, Some("libkernel"));
    assert_eq!(
        imports[0].nid,
        selfish_nid::Nid::of("sceKernelLoadStartModule")
    );

    // The adjacency the tag meanings were established from, now true of output as well. At
    // least one of the two has to have been checked, or this says nothing.
    let mut checked = 0;
    if info.jmprel != 0 && info.pltrelsz > 0 {
        assert_eq!(
            info.jmprel + info.pltrelsz,
            info.rela,
            "JMPREL + PLTRELSZ == RELA"
        );
        checked += 1;
    }
    if info.relasz > 0 {
        assert_eq!(info.rela + info.relasz, info.hash, "RELA + RELASZ == HASH");
        checked += 1;
    }
    assert!(
        checked > 0,
        "neither adjacency was checked, so this proves nothing"
    );

    // The initialiser is present because the module defines one.
    assert_eq!(info.init, init.unwrap_or(0));
}

#[test]
fn an_unclaimed_import_stops_the_build_rather_than_shipping() {
    let dir = std::env::temp_dir().join("selfish-module-unclaimed");
    let Some(bytes) = link(&dir) else {
        return;
    };

    let elf = selfish_elf::Elf::parse(&bytes).expect("a readable module");
    let sections = elf
        .sections()
        .expect("a readable section table")
        .expect("sections");
    let dynsym = sections.find(".dynsym").expect(".dynsym");
    let strings = sections
        .headers()
        .get(dynsym.link.get() as usize)
        .and_then(|header| sections.contents(header))
        .expect("its string table");

    // A manifest that claims nothing. Giving the symbol library zero instead would be a
    // valid-looking answer that resolves to nothing at run time.
    let error = dynlib::build(
        Linked {
            symbols: sections.contents(dynsym).expect("bytes"),
            names: strings,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        },
        "probe",
        &[],
        &|_| None,
    )
    .unwrap_err();

    let message = error.to_string();
    assert!(
        message.contains("sceKernelLoadStartModule"),
        "the error names what is unclaimed, since the fix is always to add it: {message}"
    );
}
