use super::{BuildError, Library, Linked, Resolution, build, elf_hash, module_version};
use crate::dynamic::{self, Table, standard};
use selfish_abi::Generation;

/// A symbol table with one null entry and then the named ones.
fn linked_symbols(names: &[(&str, u16)]) -> (Vec<u8>, Vec<u8>) {
    let mut strings = vec![0_u8];
    let mut symbols = vec![0_u8; 24];
    for (name, section) in names {
        let at = u32::try_from(strings.len()).unwrap();
        strings.extend_from_slice(name.as_bytes());
        strings.push(0);

        symbols.extend_from_slice(&at.to_le_bytes());
        symbols.push(0x10); // global binding, NOTYPE - what a linker leaves
        symbols.push(0);
        symbols.extend_from_slice(&section.to_le_bytes());
        symbols.extend_from_slice(&0_u64.to_le_bytes());
        symbols.extend_from_slice(&0_u64.to_le_bytes());
    }
    (symbols, strings)
}

/// Claim every symbol for library zero, hashing its name.
#[allow(
    clippy::unnecessary_wraps,
    reason = "it matches the resolver shape `build` takes"
)]
fn resolve_all(name: &str) -> Option<Resolution> {
    Some(Resolution {
        nid: selfish_nid::Nid::of(name),
        library: 0,
        module: 0,
    })
}

fn libraries() -> Vec<Library> {
    vec![Library {
        name: "libkernel".to_owned(),
        id: 0,
        module_id: 0,
    }]
}

/// A built segment reads back through `dynamic` with its import named and attributed.
#[test]
fn what_is_written_reads_back_through_the_reader() {
    let (symbols, names) = linked_symbols(&[("sceKernelLoadStartModule", 0), ("local", 1)]);
    let segment = build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0x1000,
        },
        "probe",
        &libraries(),
        &resolve_all,
    )
    .expect("a segment");

    let entries = segment.entries(
        Table::Orbis,
        Generation::Orbis,
        0,
        None,
        crate::ObjectType::SharedLibrary,
    );
    let info = dynamic::Info::from_entries(&entries);
    assert_eq!(info.table, Some(Table::Orbis));

    let imports = dynamic::imports(&segment.bytes, &info).expect("imports");
    assert_eq!(imports.len(), 1, "one undefined symbol, so one import");
    assert_eq!(imports[0].library, Some("libkernel"));
    assert_eq!(imports[0].module, Some("libkernel"));
    assert_eq!(
        imports[0].nid,
        selfish_nid::Nid::of("sceKernelLoadStartModule")
    );
}

/// A defined symbol keeps its plain name and is not encoded.
#[test]
fn a_defined_symbol_keeps_its_plain_name() {
    let (symbols, names) = linked_symbols(&[("local", 1)]);
    let segment = build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        },
        "probe",
        &libraries(),
        &resolve_all,
    )
    .expect("a segment");

    assert_eq!(segment.encoded, 0);
    let text = String::from_utf8_lossy(&segment.bytes);
    assert!(text.contains("local"), "the plain name survives");
}

/// An unclaimed import is an error that names the symbol.
#[test]
fn an_unclaimed_import_is_an_error_and_is_named() {
    let (symbols, names) = linked_symbols(&[("sceSomethingUnknown", 0)]);
    let error = build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        },
        "probe",
        &libraries(),
        &|_| None,
    )
    .unwrap_err();
    assert_eq!(
        error,
        BuildError::Unclaimed(vec!["sceSomethingUnknown".to_owned()])
    );
}

/// An import is written typed as a function with a global binding.
#[test]
fn an_import_is_typed_as_a_function() {
    let (symbols, names) = linked_symbols(&[("sceKernelLoadStartModule", 0)]);
    let segment = build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        },
        "probe",
        &libraries(),
        &resolve_all,
    )
    .expect("a segment");

    let entries = segment.entries(
        Table::Orbis,
        Generation::Orbis,
        0,
        None,
        crate::ObjectType::SharedLibrary,
    );
    let info = dynamic::Info::from_entries(&entries);
    let read = dynamic::symbols(&segment.bytes, &info).expect("symbols");
    // The reserved null entry also has section zero, so it is skipped by name offset.
    let import = read
        .iter()
        .find(|symbol| symbol.is_import() && symbol.name_offset != 0)
        .expect("an import");
    assert_eq!(import.kind(), super::FUNCTION);
    assert_eq!(import.binding(), 1, "and the binding is global");
}

/// Only the fingerprint precedes the string table, and every name tag follows it.
#[test]
fn the_string_table_is_declared_before_anything_that_names_a_string() {
    let (symbols, names) = linked_symbols(&[("local", 1)]);
    let segment = build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        },
        "probe",
        &libraries(),
        &resolve_all,
    )
    .expect("a segment");

    let entries = segment.entries(
        Table::Orbis,
        Generation::Orbis,
        0,
        None,
        crate::ObjectType::SharedLibrary,
    );
    let tags = dynamic::Tags::of(Table::Orbis);
    let strtab = entries.iter().position(|(tag, _)| *tag == tags.strtab);
    let module_info = entries.iter().position(|(tag, _)| *tag == tags.module_info);
    assert_eq!(
        entries.first().map(|(tag, _)| *tag),
        Some(dynamic::vendor::FINGERPRINT),
        "only the fingerprint comes before the string table"
    );
    assert_eq!(strtab, Some(1), "and the string table is next");
    assert!(module_info > strtab, "and every name comes after it");
}

/// An empty relocation table is not declared, but the relocation form always is.
#[test]
fn an_empty_relocation_table_is_not_declared_but_its_form_always_is() {
    let (symbols, names) = linked_symbols(&[("local", 1)]);
    let segment = build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        },
        "probe",
        &libraries(),
        &resolve_all,
    )
    .expect("a segment");

    let entries = segment.entries(
        Table::Orbis,
        Generation::Orbis,
        0,
        None,
        crate::ObjectType::SharedLibrary,
    );
    let tags = dynamic::Tags::of(Table::Orbis);
    assert!(entries.iter().any(|(tag, _)| *tag == tags.pltrel));
    assert!(!entries.iter().any(|(tag, _)| *tag == tags.jmprel));
    assert!(!entries.iter().any(|(tag, _)| *tag == tags.rela));
}

/// Each imported library adds `TAGS_PER_LIBRARY` dynamic entries.
#[test]
fn every_library_costs_four_tags() {
    let (symbols, names) = linked_symbols(&[("local", 1)]);
    let two = vec![
        Library {
            name: "libkernel".to_owned(),
            id: 0,
            module_id: 0,
        },
        Library {
            name: "libSceFios2".to_owned(),
            id: 1,
            module_id: 1,
        },
    ];
    let linked = Linked {
        symbols: &symbols,
        names: &names,
        jmprel: &[],
        rela: &[],
        pltgot: 0,
    };
    let one = build(linked, "probe", &libraries(), &resolve_all)
        .expect("a segment")
        .entries(
            Table::Orbis,
            Generation::Orbis,
            0,
            None,
            crate::ObjectType::SharedLibrary,
        )
        .len();
    let both = build(linked, "probe", &two, &resolve_all)
        .expect("a segment")
        .entries(
            Table::Orbis,
            Generation::Orbis,
            0,
            None,
            crate::ObjectType::SharedLibrary,
        )
        .len();
    assert_eq!(both - one, crate::layout::TAGS_PER_LIBRARY);
}

/// One segment yields different tag numbers per convention, with Prospero values as addresses.
#[test]
fn the_two_conventions_produce_different_tags_for_the_same_segment() {
    let (symbols, names) = linked_symbols(&[("local", 1)]);
    let segment = build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        },
        "probe",
        &libraries(),
        &resolve_all,
    )
    .expect("a segment");

    let orbis = segment.entries(
        Table::Orbis,
        Generation::Orbis,
        0,
        None,
        crate::ObjectType::SharedLibrary,
    );
    let prospero = segment.entries(
        Table::Prospero,
        Generation::Prospero,
        0x1000,
        None,
        crate::ObjectType::SharedLibrary,
    );
    // Compared on the string table, since the fingerprint tag is the same in both.
    let find = |entries: &[(u64, u64)], table| {
        let wanted = dynamic::Tags::of(table).strtab;
        entries
            .iter()
            .find(|(tag, _)| *tag == wanted)
            .copied()
            .expect("a string table")
    };
    let (orbis_tag, orbis_value) = find(&orbis, Table::Orbis);
    let (prospero_tag, prospero_value) = find(&prospero, Table::Prospero);
    assert_ne!(orbis_tag, prospero_tag, "different tag numbers");
    assert_eq!(
        prospero_value - orbis_value,
        0x1000,
        "and the prospero convention's values are addresses"
    );
}

/// With no initialiser, `DT_INIT` is omitted rather than written as zero.
#[test]
fn the_initialiser_is_absent_rather_than_zero_when_there_is_none() {
    let (symbols, names) = linked_symbols(&[("local", 1)]);
    let segment = build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        },
        "probe",
        &libraries(),
        &resolve_all,
    )
    .expect("a segment");

    let without = segment.entries(
        Table::Orbis,
        Generation::Orbis,
        0,
        None,
        crate::ObjectType::SharedLibrary,
    );
    let with = segment.entries(
        Table::Orbis,
        Generation::Orbis,
        0,
        Some(0x2000),
        crate::ObjectType::SharedLibrary,
    );
    assert_eq!(with.len(), without.len() + 1);
    assert!(!without.iter().any(|(tag, _)| *tag == standard::INIT));
}

/// Only a shared library declares an export library, leaving id zero to an executable's imports.
#[test]
fn an_executable_declares_no_export_library_and_a_shared_library_does() {
    let (symbols, names) = linked_symbols(&[("local", 1)]);
    let segment = build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        },
        "probe",
        &libraries(),
        &resolve_all,
    )
    .expect("a segment");

    let exports = |kind| {
        segment
            .entries(Table::Orbis, Generation::Orbis, 0, None, kind)
            .iter()
            .any(|(tag, _)| {
                *tag == dynamic::vendor::EXPORT_LIB_ORBIS
                    || *tag == dynamic::vendor::EXPORT_LIB_ATTR
            })
    };
    assert!(
        !exports(crate::ObjectType::Executable),
        "an executable exports nothing"
    );
    assert!(
        !exports(crate::ObjectType::FixedExecutable),
        "and neither does a fixed one"
    );
    assert!(
        exports(crate::ObjectType::SharedLibrary),
        "a shared library declares its export library"
    );
}

/// Each library's bare name and `.prx` filename are both in the string table.
#[test]
fn a_library_is_named_twice_because_two_tags_spell_it_differently() {
    let (symbols, names) = linked_symbols(&[("local", 1)]);
    let segment = build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        },
        "probe",
        &libraries(),
        &resolve_all,
    )
    .expect("a segment");

    let text = String::from_utf8_lossy(&segment.bytes);
    assert!(text.contains("libkernel\0"), "the bare name");
    assert!(text.contains("libkernel.prx\0"), "and the filename");
}

/// `elf_hash` is the ELF specification's hash.
#[test]
fn the_hash_is_the_one_the_specification_fixes() {
    assert_eq!(elf_hash(b""), 0);
    assert_eq!(elf_hash(b"printf"), 0x0779_05A6);
}

/// The display library declares 0.0 on Orbis and the default elsewhere, per the data table.
#[test]
fn the_display_library_declares_a_different_version_on_the_previous_generation() {
    assert_eq!(module_version("libSceVideoOut", Generation::Orbis), (0, 0));
    assert_eq!(
        module_version("libSceVideoOut", Generation::Prospero),
        (1, 1)
    );
    assert_eq!(module_version("libkernel", Generation::Orbis), (1, 1));
}

/// The generation's library version reaches the needed-module entries.
#[test]
fn the_generation_reaches_the_library_entries() {
    let (symbols, names) = linked_symbols(&[("local", 1)]);
    let video = vec![Library {
        name: "libSceVideoOut".to_owned(),
        id: 0,
        module_id: 0,
    }];
    let segment = build(
        Linked {
            symbols: &symbols,
            names: &names,
            jmprel: &[],
            rela: &[],
            pltgot: 0,
        },
        "probe",
        &video,
        &resolve_all,
    )
    .expect("a segment");

    let tags = dynamic::Tags::of(Table::Orbis);
    let version_of = |generation| {
        segment
            .entries(
                Table::Orbis,
                generation,
                0,
                None,
                crate::ObjectType::SharedLibrary,
            )
            .into_iter()
            .find(|(tag, _)| *tag == tags.needed_module)
            .map(|(_, value)| (value >> 32) & 0xFFFF)
            .expect("a needed-module entry")
    };
    assert_eq!(version_of(Generation::Orbis), 0x0000, "0.0");
    assert_eq!(version_of(Generation::Prospero), 0x0101, "1.1");
}

/// An import library's attribute is the literal `0x9`, distinct from the export attribute.
#[test]
fn an_import_librarys_attribute_is_not_the_export_attribute() {
    assert_eq!(
        super::version::IMPORT_LIBRARY,
        0x9,
        "as a launching title writes it"
    );
    assert_ne!(
        super::version::IMPORT_LIBRARY,
        super::version::AUTO_EXPORT,
        "import and export attributes differ"
    );
}

/// A `WEAK FUNC` import is rewritten to `GLOBAL FUNC` in the encoded byte.
#[test]
fn an_import_is_rebound_global_even_when_the_compiler_marked_it_weak() {
    // st_info: binding in the high nibble, type in the low one. WEAK FUNC is 0x22.
    let mut entry = [0_u8; 24];
    entry[4] = 0x22;
    super::set_type(&mut entry, super::FUNCTION).expect("type");
    super::set_binding(&mut entry, super::GLOBAL).expect("binding");
    assert_eq!(
        entry[4], 0x12,
        "GLOBAL FUNC, as a launching title writes it"
    );
}

/// The `linked_symbols` fixture with raw-byte names and an optional forced name offset.
fn linked_raw(names: &[(&[u8], u16, Option<u32>)]) -> (Vec<u8>, Vec<u8>) {
    let mut strings = vec![0_u8];
    let mut symbols = vec![0_u8; 24];
    for (name, section, forced) in names {
        let at = forced.unwrap_or_else(|| u32::try_from(strings.len()).unwrap());
        if forced.is_none() {
            strings.extend_from_slice(name);
            strings.push(0);
        }
        symbols.extend_from_slice(&at.to_le_bytes());
        symbols.push(0x10); // global binding, NOTYPE - what a linker leaves
        symbols.push(0);
        symbols.extend_from_slice(&section.to_le_bytes());
        symbols.extend_from_slice(&0_u64.to_le_bytes());
        symbols.extend_from_slice(&0_u64.to_le_bytes());
    }
    (symbols, strings)
}

fn build_raw(names: &[(&[u8], u16, Option<u32>)]) -> Result<super::Segment, BuildError> {
    let (symbols, strings) = linked_raw(names);
    build(
        Linked {
            symbols: &symbols,
            names: &strings,
            jmprel: &[],
            rela: &[],
            pltgot: 0x1000,
        },
        "probe",
        &libraries(),
        &resolve_all,
    )
}

/// A symbol name that is not UTF-8 is refused rather than rewritten lossily.
#[test]
fn a_symbol_name_that_is_not_utf8_is_refused_rather_than_rewritten() {
    let refused = build_raw(&[(&[0x6c, 0x6f, 0xff, 0x63], 1, None)]);
    assert!(
        matches!(refused, Err(BuildError::SymbolName(_))),
        "expected a refusal, got {refused:?}"
    );
}

/// A name offset past the end is refused rather than read as an empty, defined name.
#[test]
fn a_name_offset_past_the_end_is_refused_rather_than_read_as_no_name() {
    let refused = build_raw(&[(b"whatever", 0, Some(9999))]);
    assert!(
        matches!(refused, Err(BuildError::SymbolName(_))),
        "expected a refusal, got {:?}",
        refused.map(|segment| segment.encoded)
    );
}

/// An unterminated name is refused rather than read to the end of the table.
#[test]
fn an_unterminated_name_is_refused_rather_than_swallowing_the_rest_of_the_table() {
    let (symbols, _) = linked_raw(&[(b"tail", 1, Some(1))]);
    let refused = build(
        Linked {
            symbols: &symbols,
            names: b"\0unterminated",
            jmprel: &[],
            rela: &[],
            pltgot: 0x1000,
        },
        "probe",
        &libraries(),
        &resolve_all,
    );
    assert!(
        matches!(refused, Err(BuildError::SymbolName(_))),
        "expected a refusal, got {refused:?}"
    );
}
