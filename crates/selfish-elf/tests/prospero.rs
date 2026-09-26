//! The Prospero convention, written by `dynlib` and read back through `Elf::tables`.
//!
//! This checks the writer and reader agree on tag numbers, the virtual-address origin, the
//! mapped segment and the rebasing; it is not checked against a Prospero-generation module.
//! Skipped rather than failed without `clang` and `ld.lld`, as in `links.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "a panic in a test is the test failing"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use selfish_abi::Generation;
use selfish_elf::dynamic::{self, Table};
use selfish_elf::dynlib::{self, Library, Linked, Resolution};

const SOURCE: &str = r#"
__attribute__((section(".sce_process_param"))) const unsigned long param[4] = {0};
__attribute__((section(".sce_dynlibdata"))) const unsigned char dynlib[1] = {0};
__attribute__((section(".interp"))) const char interp[] = "/system/common/lib/libkernel.sprx";

extern int sceKernelLoadStartModule(const char *name);
extern int sceVideoOutOpen(int user, int type, int index, const void *param);
const char module_name[] = "probe.prx";

int _start(void) {
    return sceKernelLoadStartModule(module_name) + sceVideoOutOpen(0, 0, 0, 0);
}
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

    assert!(
        Command::new("clang")
            .args(["--target=x86_64-unknown-none", "-ffreestanding"])
            .args(["-fno-stack-protector", "-fPIC", "-c"])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .status()
            .expect("running clang")
            .success(),
        "clang failed"
    );
    assert!(
        Command::new("ld.lld")
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
            .status()
            .expect("running ld.lld")
            .success(),
        "the script did not link"
    );
    Some(std::fs::read(&linked).expect("the linked module"))
}

/// Build a module under one convention in place and return what `install` did.
fn build(bytes: &mut Vec<u8>, table: Table, generation: Generation) -> dynlib::Installed {
    let (symbols, names, jmprel, rela, pltgot) = {
        let elf = selfish_elf::Elf::parse(bytes).expect("a readable module");
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
        let contents = |name: &str| {
            sections
                .find(name)
                .and_then(|header| sections.contents(header))
                .unwrap_or_default()
                .to_vec()
        };
        (
            sections.contents(dynsym).expect("bytes").to_vec(),
            strings.to_vec(),
            contents(".rela.plt"),
            contents(".rela.dyn"),
            sections
                .find(".got.plt")
                .or_else(|| sections.find(".got"))
                .map_or(0, |header| header.addr.get()),
        )
    };

    // Two libraries with distinct ids; the display library has a row in
    // `data/library-versions.tsv`.
    let libraries = vec![
        Library {
            name: "libkernel".to_owned(),
            id: 0,
            module_id: 0,
        },
        Library {
            name: "libSceVideoOut".to_owned(),
            id: 1,
            module_id: 1,
        },
    ];
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
            let (library, module) = match name {
                "sceKernelLoadStartModule" => (0, 0),
                "sceVideoOutOpen" => (1, 1),
                _ => return None,
            };
            Some(Resolution {
                nid: selfish_nid::Nid::with_suffix(name, &selfish_nid::suffix()),
                library,
                module,
            })
        },
    )
    .expect("a vendor segment");
    assert_eq!(segment.encoded, 2, "both imports were re-encoded");

    let installed =
        dynlib::install(bytes, &segment, table, generation, None).expect("an installed module");
    selfish_elf::identity::stamp(bytes, selfish_elf::ObjectType::Executable, generation)
        .expect("stamped");
    installed
}

/// A Prospero-convention module maps its tables and reads back with rebased offsets.
#[test]
fn a_prospero_convention_module_reads_back_through_the_reader() {
    let dir = std::env::temp_dir().join("selfish-prospero-test");
    let Some(mut bytes) = link(&dir) else {
        return;
    };
    let installed = build(&mut bytes, Table::Prospero, Generation::Prospero);

    // The tables are mapped, so the tags hold virtual addresses in an ordinary `PT_LOAD`.
    assert_ne!(
        installed.table_base, 0,
        "the prospero convention places the tables in the address space"
    );

    let elf = selfish_elf::Elf::parse(&bytes).expect("the rebuilt module");
    assert_eq!(elf.generation(), Some(Generation::Prospero));
    assert!(
        elf.vendor_segment().is_none(),
        "there is no PT_SCE_DYNLIBDATA under this convention"
    );

    let (segment, info) = elf
        .tables()
        .expect("a readable dynamic table")
        .expect("a module that carries vendor tables");
    assert_eq!(info.table, Some(Table::Prospero));

    // Rebased: the tags held addresses and what comes back are offsets into the segment.
    assert!(
        info.strtab < segment.len() as u64,
        "strtab {:#x} should be an offset into a {:#x}-byte segment, not an address",
        info.strtab,
        segment.len()
    );

    let imports = dynamic::imports(segment, &info).expect("imports");
    assert_eq!(imports.len(), 2);
    let mut named: Vec<_> = imports
        .iter()
        .map(|import| import.library.unwrap_or("<unnamed>"))
        .collect();
    named.sort_unstable();
    assert_eq!(named, ["libSceVideoOut", "libkernel"]);
    assert!(
        imports
            .iter()
            .any(|import| import.nid == selfish_nid::Nid::of("sceVideoOutOpen")),
        "the hash survives the convention it was written under"
    );
}

/// One source built under both conventions has different tags and identical imports.
#[test]
fn the_two_conventions_disagree_about_the_bytes_and_agree_about_the_meaning() {
    let dir = std::env::temp_dir().join("selfish-conventions-both");
    let Some(linked) = link(&dir) else {
        return;
    };

    let mut orbis = linked.clone();
    build(&mut orbis, Table::Orbis, Generation::Orbis);
    let mut prospero = linked;
    build(&mut prospero, Table::Prospero, Generation::Prospero);

    let read = |bytes: &[u8]| {
        let elf = selfish_elf::Elf::parse(bytes).expect("a module");
        let entries = elf.dynamic_entries().expect("a dynamic table");
        let (segment, info) = elf.tables().expect("readable").expect("vendor tables");
        let mut names: Vec<String> = dynamic::imports(segment, &info)
            .expect("imports")
            .iter()
            .map(|import| {
                format!(
                    "{} {}",
                    import.nid.encode(),
                    import.library.unwrap_or("<unnamed>")
                )
            })
            .collect();
        names.sort();
        (entries, names)
    };

    let (orbis_entries, orbis_imports) = read(&orbis);
    let (prospero_entries, prospero_imports) = read(&prospero);

    assert_eq!(
        orbis_imports, prospero_imports,
        "the same module, so the same imports"
    );
    assert!(!orbis_imports.is_empty(), "and there are some to compare");

    let tags = |entries: &[(u64, u64)]| {
        entries
            .iter()
            .map(|(tag, _)| *tag)
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_ne!(
        tags(&orbis_entries),
        tags(&prospero_entries),
        "the conventions use different tag numbers"
    );
}

/// The display library is declared 0.0 on Orbis and 1.1 on Prospero in a built module.
#[test]
fn the_display_library_gets_its_measured_version_only_on_the_orbis_generation() {
    let dir = std::env::temp_dir().join("selfish-display-version");
    let Some(linked) = link(&dir) else {
        return;
    };

    let version_for = |generation| {
        let mut bytes = linked.clone();
        build(&mut bytes, Table::Orbis, generation);
        let elf = selfish_elf::Elf::parse(&bytes).expect("a module");
        let entries = elf.dynamic_entries().expect("a dynamic table");
        let tags = dynamic::Tags::of(Table::Orbis);
        entries
            .iter()
            .filter(|(tag, _)| *tag == tags.needed_module)
            // Library id 1 is `libSceVideoOut`; id 0 is the kernel and always 1.1.
            .find(|(_, value)| (value >> 48) == 1)
            .map(|(_, value)| (value >> 32) & 0xFFFF)
            .expect("a needed-module entry for the display library")
    };

    assert_eq!(version_for(Generation::Orbis), 0x0000, "0.0");
    assert_eq!(version_for(Generation::Prospero), 0x0101, "1.1");
}
