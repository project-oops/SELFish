//! `nid`, `elf`, `imports`, `sections` and `reloc`: views of one executable.

use std::borrow::Cow;
use std::path::Path;

use crate::Result;

/// `selfish nid`: each name's import hash.
pub(crate) fn nid(names: &[String]) -> Result {
    if names.is_empty() {
        return Err("give at least one symbol name".into());
    }
    for name in names {
        say!("{}  {name}", selfish_nid::Nid::of(name));
    }
    Ok(())
}

/// Read a file and reach the executable, whether or not it is wrapped in a container.
///
/// A `.prx` or an `eboot.bin` is usually a container, and what it imports is the same question
/// either way, so a container is unwrapped rather than refused.
fn read_executable(path: &Path) -> Result<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    Ok(unwrap_container(&bytes).into_owned())
}

fn unwrap_container(bytes: &[u8]) -> Cow<'_, [u8]> {
    let Ok(container) = selfish_container::Container::parse(bytes) else {
        return Cow::Borrowed(bytes);
    };
    match container.to_elf() {
        Ok(inner) => {
            say!("container  {}, unwrapped", container.generation());
            Cow::Owned(inner)
        }
        Err(_) => Cow::Borrowed(bytes),
    }
}

/// `selfish elf`: header, segments and the dynamic table.
pub(crate) fn describe(path: &Path) -> Result {
    let bytes = read_executable(path)?;
    let elf = selfish_elf::Elf::parse(&bytes)?;
    say!("type       {}", elf.object_type());
    say!(
        "generation {}",
        elf.generation()
            .map_or_else(|| "neither".to_owned(), |g| g.to_string())
    );
    say!(
        "osabi      {}",
        if elf.has_platform_osabi() {
            "FreeBSD, as the platform requires"
        } else {
            "NOT FreeBSD - a loader refuses this before anything else"
        }
    );
    say!("entry      {:#x}", elf.entry());
    say!("segments   {}", elf.program_headers().len());
    for (i, phdr) in elf.program_headers().iter().enumerate() {
        let kind = phdr.p_type.get();
        say!(
            "  [{i}] {kind:#010x}{}  flags={:#x} off={:#010x} va={:#012x} fsz={:#010x} msz={:#010x} align={:#x}",
            if selfish_elf::segment::is_vendor(kind) {
                " vendor"
            } else {
                "       "
            },
            phdr.flags.get(),
            phdr.offset.get(),
            phdr.vaddr.get(),
            phdr.filesz.get(),
            phdr.memsz.get(),
            phdr.align.get()
        );
    }
    describe_dynamic(&elf)
}

fn describe_dynamic(elf: &selfish_elf::Elf<'_>) -> Result {
    use selfish_elf::dynamic;

    let entries = elf.dynamic_entries()?;
    if entries.is_empty() {
        return Ok(());
    }
    let info = dynamic::Info::from_entries(&entries);
    say!();
    say!("dynamic    {} entries", entries.len());
    say!(
        "convention {}",
        match info.table {
            Some(dynamic::Table::Orbis) => "orbis - vendor tags throughout",
            Some(dynamic::Table::Prospero) => "prospero - standard tags, vendor extras",
            None => "no string table, so undetermined",
        }
    );
    say!("strtab     {:#x}, {} bytes", info.strtab, info.strsz);
    say!("symtab     {:#x}, entry {:#x}", info.symtab, info.syment);
    if let Some(count) = info.symbol_count() {
        say!("symbols    {count}");
    }
    say!(
        "libraries  {} vendor import entries, {} DT_NEEDED",
        info.import_libs.len(),
        info.needed.len()
    );
    let Some(strings) = elf.vendor_segment() else {
        return Ok(());
    };
    // The tags' string offsets are relative to the vendor segment, not the file.
    let base = usize::try_from(info.strtab).unwrap_or(0);
    let table = strings.get(base..).unwrap_or_default();
    for packed in info.import_libs.iter().take(8) {
        let (id, offset) = dynamic::split_table_entry(*packed);
        match dynamic::string_at(table, offset) {
            Ok(name) => say!("  library {id:>3}  {name}"),
            Err(error) => say!("  library {id:>3}  <{error}>"),
        }
    }
    if info.import_libs.len() > 8 {
        say!(
            "  ... and {} more",
            info.import_libs.len().saturating_sub(8)
        );
    }
    Ok(())
}

/// `selfish imports`: every import, or a count per library.
pub(crate) fn imports(path: &Path, all: bool) -> Result {
    let bytes = read_executable(path)?;
    let elf = selfish_elf::Elf::parse(&bytes)?;
    let Some((segment, info)) = elf.tables()? else {
        say!("no vendor tables, so nothing to resolve");
        return Ok(());
    };
    let imports = selfish_elf::dynamic::imports(segment, &info)?;
    say!("imports    {}", imports.len());

    if all {
        for import in &imports {
            say!(
                "  {}  {:<28} {}",
                import.nid.encode(),
                import.library.unwrap_or("<unnamed>"),
                import.module.unwrap_or("<unnamed>")
            );
        }
        return Ok(());
    }

    let mut counts: Vec<(&str, &str, usize)> = Vec::new();
    for import in &imports {
        let library = import.library.unwrap_or("<unnamed>");
        let module = import.module.unwrap_or("<unnamed>");
        match counts
            .iter_mut()
            .find(|(seen, from, _)| *seen == library && *from == module)
        {
            Some((_, _, count)) => *count = count.saturating_add(1),
            None => counts.push((library, module, 1)),
        }
    }
    counts.sort_unstable_by_key(|(_, _, count)| core::cmp::Reverse(*count));
    for (library, module, count) in counts {
        say!("  {count:>6}  {library:<28} {module}");
    }
    Ok(())
}

/// `selfish sections`: the section headers and link-time symbols of an unfinished module.
pub(crate) fn sections(path: &Path, defines: &[String]) -> Result {
    let bytes = read_executable(path)?;
    let elf = selfish_elf::Elf::parse(&bytes)?;
    let Some(table) = elf.sections()? else {
        say!("no sections - which is what a finished module looks like");
        return Ok(());
    };
    say!("sections   {}", table.headers().len());
    for header in table.headers() {
        say!(
            "  {:<20} type {:<3} {:>10} bytes at {:#x}",
            table.name(header).unwrap_or("<unnamed>"),
            header.kind.get(),
            header.size.get(),
            header.offset.get()
        );
    }
    if let Some((symbols, _)) = table.symbols() {
        let here = symbols
            .iter()
            .filter(|symbol| !symbol.is_undefined())
            .count();
        say!(
            "symbols    {} in .symtab, {here} defined here",
            symbols.len()
        );
    }
    for name in defines {
        say!(
            "  defines {name}: {}",
            if table.defines(name) { "yes" } else { "no" }
        );
    }
    Ok(())
}

/// `selfish reloc`: relocations by type, and how they join to the imports.
///
/// Every PLT slot names an imported function; a slot whose symbol is not an import has no
/// address for a loader to write. An import in neither table is one nothing asks for, which
/// would mean the symbol filter is wrong.
pub(crate) fn reloc(path: &Path) -> Result {
    let bytes = read_executable(path)?;
    let elf = selfish_elf::Elf::parse(&bytes)?;
    let Some((segment, info)) = elf.tables()? else {
        say!("no vendor tables, so nothing to read");
        return Ok(());
    };
    let tables = selfish_elf::dynamic::relocations(segment, &info);
    let imports = selfish_elf::dynamic::imports(segment, &info)?;
    let joined = tables
        .plt
        .iter()
        .filter(|entry| {
            imports
                .iter()
                .any(|import| import.index == entry.symbol_index())
        })
        .count();

    for (label, entries) in [("data", &tables.data), ("plt ", &tables.plt)] {
        say!("{label}  {} entries", entries.len());
        for (kind, count) in selfish_elf::reloc::census(entries) {
            let name = selfish_elf::reloc::kind::name(kind)
                .map_or_else(|| format!("unknown {kind:#x}"), str::to_owned);
            say!("  {count:>8}  {name}");
        }
    }
    let orphans: Vec<_> = imports
        .iter()
        .filter(|import| {
            !tables
                .plt
                .iter()
                .chain(tables.data.iter())
                .any(|entry| entry.needs_symbol() && entry.symbol_index() == import.index)
        })
        .collect();

    say!(
        "join  {joined} of {} PLT slots name one of {} imports, {} referenced by neither table",
        tables.plt.len(),
        imports.len(),
        orphans.len()
    );
    for import in &orphans {
        say!(
            "  unreferenced  {}  {}",
            import.nid.encode(),
            import.library.unwrap_or("<unnamed>")
        );
    }
    Ok(())
}
