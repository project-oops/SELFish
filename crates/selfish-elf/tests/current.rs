//! The current convention, written and read back.
//!
//! Every real module this repository has been pointed at is previous-generation, so
//! `Elf::tables`'s current-convention branch was **reasoned rather than measured** - code that
//! had never been executed against anything. That is the shape of defect this project keeps
//! finding in other people's work, and leaving one in is not defensible.
//!
//! It cannot be fixed with material nobody has. It *can* be fixed with the other half of the
//! crate: the writer can produce a current-convention module, and the reader can be made to
//! read it. That is principle 4 doing the job it exists for - the two halves check each other
//! where reality is unavailable.
//!
//! # What this proves and what it does not
//!
//! It proves the two halves agree: the tag numbers, the virtual-address origin, the mapped
//! segment, and the rebasing all round-trip. It does **not** prove a console agrees, because
//! nothing here has seen a current-generation module. If one ever turns up, this is the test
//! to point at it.
//!
//! Skipped rather than failed without `clang` and `ld.lld`; see `links.rs` for why.

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

/// Build a module under one convention and hand back its bytes.
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

    // Two libraries with distinct ids, so the library and module tables have something to get
    // wrong. One of them is the display library, which is the row in
    // `data/library-versions.tsv` - see the version assertion below.
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

#[test]
fn a_current_convention_module_reads_back_through_the_reader() {
    let dir = std::env::temp_dir().join("selfish-current-test");
    let Some(mut bytes) = link(&dir) else {
        return;
    };
    let installed = build(&mut bytes, Table::Current, Generation::Current);

    // The half that distinguishes the conventions: the tables are *mapped*, so the tags hold
    // virtual addresses rather than offsets and the segment is an ordinary `PT_LOAD`.
    assert_ne!(
        installed.table_base, 0,
        "the current convention places the tables in the address space"
    );

    let elf = selfish_elf::Elf::parse(&bytes).expect("the rebuilt module");
    assert_eq!(elf.generation(), Some(Generation::Current));
    assert!(
        elf.vendor_segment().is_none(),
        "there is no PT_SCE_DYNLIBDATA under this convention, which is why `tables` exists"
    );

    let (segment, info) = elf
        .tables()
        .expect("a readable dynamic table")
        .expect("a module that carries vendor tables");
    assert_eq!(info.table, Some(Table::Current));

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

#[test]
fn the_two_conventions_disagree_about_the_bytes_and_agree_about_the_meaning() {
    // The same source, built both ways. Every tag number and every table value differs; the
    // imports that come back out are identical. That is the whole claim of having two
    // conventions in one crate, and nothing else in the suite states it end to end.
    let dir = std::env::temp_dir().join("selfish-current-both");
    let Some(linked) = link(&dir) else {
        return;
    };

    let mut legacy = linked.clone();
    build(&mut legacy, Table::Legacy, Generation::Previous);
    let mut current = linked;
    build(&mut current, Table::Current, Generation::Current);

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

    let (legacy_entries, legacy_imports) = read(&legacy);
    let (current_entries, current_imports) = read(&current);

    assert_eq!(
        legacy_imports, current_imports,
        "the same module, so the same imports"
    );
    assert!(!legacy_imports.is_empty(), "and there are some to compare");

    let tags = |entries: &[(u64, u64)]| {
        entries
            .iter()
            .map(|(tag, _)| *tag)
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_ne!(
        tags(&legacy_entries),
        tags(&current_entries),
        "the conventions use different tag numbers, which is the thing that gets confused"
    );
}

#[test]
fn the_display_library_gets_its_measured_version_only_on_the_previous_generation() {
    // `data/library-versions.tsv` has exactly one row and this is what it is for: declaring
    // 1.1 for this library on the previous generation binds a module to the current
    // generation's registration, which has no way to present a frame. The module runs and
    // draws nothing, which is the worst kind of wrong.
    let dir = std::env::temp_dir().join("selfish-current-version");
    let Some(linked) = link(&dir) else {
        return;
    };

    let version_for = |generation| {
        let mut bytes = linked.clone();
        build(&mut bytes, Table::Legacy, generation);
        let elf = selfish_elf::Elf::parse(&bytes).expect("a module");
        let entries = elf.dynamic_entries().expect("a dynamic table");
        let tags = dynamic::Tags::of(Table::Legacy);
        entries
            .iter()
            .filter(|(tag, _)| *tag == tags.needed_module)
            // Library id 1 is `libSceVideoOut`; id 0 is the kernel and always 1.1.
            .find(|(_, value)| (value >> 48) == 1)
            .map(|(_, value)| (value >> 32) & 0xFFFF)
            .expect("a needed-module entry for the display library")
    };

    assert_eq!(version_for(Generation::Previous), 0x0000, "0.0");
    assert_eq!(version_for(Generation::Current), 0x0101, "1.1");
}
