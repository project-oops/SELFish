//! Command-line access to the console's file formats.
//!
//! A thin shim. Every subcommand is a few lines over a library crate, and none of them holds
//! logic of its own - if a command needs a behaviour, that behaviour belongs in the crate so
//! the other two consumers get it too.
//!
//! The point of having it at all is that a library nobody can invoke cannot be checked
//! against real material, and real material is the only oracle these formats have.

#![forbid(unsafe_code)]

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use selfish_abi::Generation;
use selfish_pfs::{Compressed, Filesystem, Region, Slice, Source, Xts};

#[derive(Parser)]
#[command(
    name = "selfish",
    version = oops_build::line!(),
    about = "Read and write the file formats Prospero-generation hardware loads"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Hash a symbol name the way a loader does.
    Nid {
        /// The symbol names.
        names: Vec<String>,
    },
    /// Describe an executable.
    Elf {
        /// The file.
        file: PathBuf,
    },
    /// List what an executable imports, resolved to library and module names.
    Imports {
        /// The file.
        file: PathBuf,
        /// Show every import rather than a summary by library.
        #[arg(long)]
        all: bool,
    },
    /// Stamp the platform identity a loader checks before it reads anything else.
    ///
    /// No linker sets these, because no linker knows about either console.
    Stamp {
        /// The module, rewritten in place.
        file: PathBuf,
        /// Console generation: 5 for the current one, 4 for the previous.
        #[arg(long, default_value_t = 4)]
        generation: u8,
        /// Stamp it as a shared library rather than an executable.
        #[arg(long)]
        library: bool,
    },
    /// List an object's sections and its link-time symbol table.
    Sections {
        /// The file.
        file: PathBuf,
        /// Also report whether these symbols are defined.
        #[arg(long)]
        defines: Vec<String>,
    },
    /// Census a module's relocation tables by type.
    Reloc {
        /// The file.
        file: PathBuf,
    },
    /// Describe a container, or an executable inside one.
    Container {
        /// The file.
        file: PathBuf,
    },
    /// Wrap an executable in a container.
    Wrap {
        /// The executable to wrap.
        file: PathBuf,
        /// Where to write it. Defaults to `eboot.bin` beside the input.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Console generation: 5 for the current one, 4 for the previous.
        ///
        /// **4 is the default, and that is a measurement rather than a habit.** Every
        /// container found inside real packages for the current console carries the previous
        /// generation's magic - thirty-three of them, including a working homebrew store.
        #[arg(long, default_value_t = 4)]
        generation: u8,
    },
    /// Show what a title says about itself.
    ///
    /// Takes a package, a `PARAM.SFO`, or a `param.json`, and works out which it was given.
    Title {
        /// The file.
        file: PathBuf,
        /// Write the metadata back out and check it matches byte for byte.
        #[arg(long)]
        round_trip: bool,
    },
    /// Check a real container against the format table, and say which rows it settles.
    ///
    /// `data/self-format.tsv` is derived from previous-generation sources and says so: every
    /// row is a hypothesis until a current-generation file confirms or refutes it. Point this
    /// at a real `eboot.bin` - dumped off a console - and it reports which fixed header rows the
    /// real file agrees with, and which it contradicts. A contradiction is a finding, not a new
    /// fact: settling what a field means at a new generation needs a citable source, not this
    /// file. (D084)
    ///
    /// The input may be a whole SELF or just its header region - a few kilobytes is enough,
    /// which is what an on-console probe can afford to dump into a report.
    Audit {
        /// The container, or a dump of its header region.
        file: PathBuf,
    },
    /// Lay out a **native title directory**, the current generation's own title format.
    ///
    /// # This is not a package, and it is not an executable title
    ///
    /// A package installs through the previous generation's compatibility path, which is why
    /// homebrew shipped that way is badged as the older hardware. The native path is a
    /// directory under `/user/app/<TITLE_ID>/` described by `param.json`, registered by
    /// `sceAppInstUtilAppInstallTitleDir` - and *that* is what produces a current-generation
    /// entry on the home screen.
    ///
    /// **It does not make a title that runs native code.** That would need a signed
    /// `eboot.bin`, and no fake-signing keyset exists for this generation the way one does for
    /// the previous. What this produces is a home-screen entry; the code behind it runs as a
    /// payload, which is already outside the compatibility sandbox.
    ///
    /// Installing it needs kernel privileges, so a payload does the copying - this only lays
    /// out the bytes.
    Native {
        /// Where to write the title directory. A `<TITLE_ID>` folder is created inside it.
        #[arg(long, short)]
        out: PathBuf,
        /// The title id, such as `OBSC00001`.
        #[arg(long)]
        title_id: String,
        /// What the title is called on the home screen.
        #[arg(long)]
        title: String,
        /// Optional content ID (such as `UP0000-PPSA01650_00-YOUTUBE000000000`).
        #[arg(long)]
        content_id: Option<String>,
        /// Content version string (such as `01.00`).
        #[arg(long)]
        version: Option<String>,
        /// What launching the icon should open. A payload's own local server, usually.
        #[arg(long)]
        deeplink: Option<String>,
        /// An icon. One is generated if this is not given.
        #[arg(long)]
        icon: Option<PathBuf>,
        /// Extra files to place in the title directory, copied verbatim.
        #[arg(long)]
        root: Option<PathBuf>,
        /// The category a title declares. Defaults to what a native homebrew entry uses.
        #[arg(long, default_value_t = NATIVE_CATEGORY)]
        category: i64,
    },
    /// Build a filesystem image from a directory of files.
    ///
    /// The image a package carries, as its own step: the files become a plain filesystem,
    /// wrapped in a `PFSC` container, carried as the single file of a signed and encrypted
    /// outer filesystem.
    ///
    /// **The content id is not optional.** The image is encrypted under a key derived from it
    /// and the passcode, so an image built under one id cannot be opened by a package built
    /// under another - and nothing about the resulting file looks wrong until a console tries
    /// to mount it.
    Image {
        /// The directory to build from - the root of what the title mounts.
        #[arg(long)]
        root: PathBuf,
        /// Where to write the image.
        #[arg(long, short)]
        out: PathBuf,
        /// The content id the image is keyed to. Must match the package that will carry it.
        #[arg(long)]
        content_id: String,
        /// The passcode. Defaults to the fake one.
        #[arg(long)]
        passcode: Option<String>,
    },
    /// Assemble a package, as far as what is established allows.
    ///
    /// Everything derivable is computed. The entries nothing here can compute must be handed
    /// in with `--entry`, and the build refuses rather than inventing them.
    Pack {
        /// The filesystem image, already built. Use `--dir` instead to build one here.
        #[arg(long, conflicts_with = "dir", required_unless_present = "dir")]
        image: Option<PathBuf>,
        /// A directory of files to build the image from - the root of what the title mounts.
        ///
        /// This is the whole chain: the files become a plain filesystem, wrapped in a `PFSC`
        /// container, carried as the single file of a signed and encrypted outer filesystem,
        /// which becomes the package's image. Nothing about it needs a key that cannot be
        /// computed from the content id and the passcode.
        #[arg(long)]
        dir: Option<PathBuf>,
        /// The passcode the package is keyed with. Defaults to the fake one.
        #[arg(long)]
        passcode: Option<String>,
        /// Where to write the package.
        #[arg(long, short)]
        out: PathBuf,
        /// The content id, such as `UP0000-TEST00001_00-0000000000000000`.
        #[arg(long, default_value = "")]
        content_id: String,
        /// An entry this crate cannot compute, as `ID=FILE` - for example `0x400=blob.bin`.
        #[arg(long = "entry", value_name = "ID=FILE")]
        entries: Vec<String>,
        /// The title id, such as `OBSC00001`. Used to generate a `param.sfo` if none is given.
        #[arg(long)]
        title_id: Option<String>,
        /// What the title is called. Used to generate a `param.sfo` if none is given.
        #[arg(long)]
        title: Option<String>,
        /// The version, as `NN.NN`.
        #[arg(long, default_value = "01.00")]
        version: String,
    },
    /// Re-derive what a package's entries mean, from packages you supply.
    ///
    /// Two of the fourteen were established by derivation rather than taken from a source.
    /// This re-runs that derivation in front of you against any packages you have, so nothing
    /// in the format table has to be taken on trust.
    Derive {
        /// The packages. More is better; two is a coincidence.
        files: Vec<PathBuf>,
    },
    /// List what is inside a package.
    Pkg {
        /// The package.
        file: PathBuf,
        /// Show every file rather than the first forty.
        #[arg(long)]
        all: bool,
    },
    /// Extract a package's files.
    Extract {
        /// The package.
        file: PathBuf,
        /// Where to write them.
        out: PathBuf,
    },
}

/// Print a line, and exit quietly when the reader has gone away.
///
/// # Why this is not `println!`
///
/// `println!` panics on a closed pipe, so `selfish imports big.prx | head` ends in a Rust
/// backtrace rather than in the four lines that were asked for. That is not cosmetic here:
/// this binary exists to be pointed at real material, and paging its output is the normal way
/// to look at it.
///
/// Exiting zero rather than propagating: a reader that stopped reading got what it wanted, and
/// a non-zero status would make every `| head` look like a failed command in a script.
///
/// The usual fix is to restore the default `SIGPIPE` handler, which needs `unsafe` and a libc
/// dependency. This crate forbids the first and does not want the second for one signal.
macro_rules! say {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let mut out = std::io::stdout().lock();
        if writeln!(out, $($arg)*).is_err() {
            std::process::exit(0);
        }
    }};
}

mod icon;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Held for the whole of `main`: the guard keeps the writers alive, and `let _` would drop
    // it here.
    let _logging = oops_log::Logging::new("selfish")
        .build(oops_build::line!())
        .init();
    match Cli::parse().command {
        Command::Nid { names } => nid(&names),
        Command::Elf { file } => elf(&file),
        Command::Imports { file, all } => imports(&file, all),
        Command::Stamp {
            file,
            generation,
            library,
        } => stamp(&file, generation, library),
        Command::Sections { file, defines } => sections(&file, &defines),
        Command::Reloc { file } => reloc(&file),
        Command::Container { file } => container(&file),
        Command::Wrap {
            file,
            out,
            generation,
        } => wrap(&file, out.as_deref(), generation),
        Command::Title { file, round_trip } => title(&file, round_trip),
        Command::Audit { file } => audit_cmd(&file),
        Command::Native {
            out,
            title_id,
            title,
            content_id,
            version,
            deeplink,
            icon,
            root,
            category,
        } => native(
            &out,
            &title_id,
            &title,
            content_id.as_deref(),
            version.as_deref(),
            deeplink.as_deref(),
            icon.as_deref(),
            root.as_deref(),
            category,
        ),
        Command::Image {
            root,
            out,
            content_id,
            passcode,
        } => image_cmd(&root, &out, &content_id, passcode.as_deref()),
        Command::Pack {
            image,
            dir,
            passcode,
            out,
            content_id,
            entries,
            title_id,
            title,
            version,
        } => pack(
            image.as_deref(),
            dir.as_deref(),
            passcode.as_deref(),
            &out,
            &content_id,
            &entries,
            title_id.as_deref(),
            title.as_deref(),
            &version,
        ),
        Command::Derive { files } => derive(&files),
        Command::Pkg { file, all } => pkg(&file, all),
        Command::Extract { file, out } => extract(&file, &out),
    }
}

fn nid(names: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if names.is_empty() {
        return Err("give at least one symbol name".into());
    }
    for name in names {
        say!("{}  {name}", selfish_nid::Nid::of(name));
    }
    Ok(())
}

fn elf(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let bytes = unwrap_container(&bytes);
    let bytes = bytes.as_ref();
    let elf = selfish_elf::Elf::parse(bytes)?;
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
    for phdr in elf.program_headers() {
        let kind = phdr.p_type.get();
        say!(
            "  {kind:#010x}{}  {:#012x} {:>10} bytes",
            if selfish_elf::segment::is_vendor(kind) {
                " vendor"
            } else {
                "       "
            },
            phdr.vaddr.get(),
            phdr.filesz.get()
        );
    }

    let entries = elf.dynamic_entries()?;
    if entries.is_empty() {
        return Ok(());
    }
    let info = selfish_elf::dynamic::Info::from_entries(&entries);
    say!();
    say!("dynamic    {} entries", entries.len());
    say!(
        "convention {}",
        match info.table {
            Some(selfish_elf::dynamic::Table::Legacy) => "legacy - vendor tags throughout",
            Some(selfish_elf::dynamic::Table::Current) => "current - standard tags, vendor extras",
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
    if let Some(strings) = elf.vendor_segment() {
        // Names come out of the vendor segment the tags point into, and the offsets are
        // relative to it rather than to the file.
        let base = usize::try_from(info.strtab).unwrap_or(0);
        let table = strings.get(base..).unwrap_or_default();
        for packed in info.import_libs.iter().take(8) {
            let (id, offset) = selfish_elf::dynamic::split_table_entry(*packed);
            match selfish_elf::dynamic::string_at(table, offset) {
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
    }
    Ok(())
}

/// Reach the executable, whether or not somebody wrapped it.
///
/// A `.prx` or an `eboot.bin` is usually a container rather than a bare executable, and being
/// handed one is the common case rather than a mistake. Unwrapped rather than refused: the
/// question "what does this module import" has the same answer either way.
fn unwrap_container(bytes: &[u8]) -> Cow<'_, [u8]> {
    match selfish_container::Container::parse(bytes) {
        Ok(container) => match container.to_elf() {
            Ok(inner) => {
                say!("container  {}, unwrapped", container.generation());
                Cow::Owned(inner)
            }
            Err(_) => Cow::Borrowed(bytes),
        },
        Err(_) => Cow::Borrowed(bytes),
    }
}

fn imports(path: &Path, all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let unwrapped = unwrap_container(&bytes);
    let elf = selfish_elf::Elf::parse(unwrapped.as_ref())?;
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

    // Grouped, because the interesting fact about a module with thousands of imports is
    // which libraries it leans on, not the order the table happens to list them in.
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

fn stamp(path: &Path, generation: u8, library: bool) -> Result<(), Box<dyn std::error::Error>> {
    let generation = match generation {
        5 => Generation::Current,
        4 => Generation::Previous,
        other => return Err(format!("generation {other} is neither 4 nor 5").into()),
    };
    let kind = if library {
        selfish_elf::ObjectType::SharedLibrary
    } else {
        selfish_elf::ObjectType::Executable
    };

    let mut bytes = std::fs::read(path)?;
    let changes = selfish_elf::identity::stamp(&mut bytes, kind, generation)?;
    if changes.is_empty() {
        say!("already stamped for {generation}, as {kind}");
        return Ok(());
    }
    for change in &changes {
        say!(
            "  {:<14} {:#x} -> {:#x}",
            change.field,
            change.from,
            change.to
        );
    }
    std::fs::write(path, &bytes)?;
    say!("{} field(s) written to {}", changes.len(), path.display());
    Ok(())
}

fn sections(path: &Path, defines: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let unwrapped = unwrap_container(&bytes);
    let elf = selfish_elf::Elf::parse(unwrapped.as_ref())?;

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

fn reloc(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let unwrapped = unwrap_container(&bytes);
    let elf = selfish_elf::Elf::parse(unwrapped.as_ref())?;
    let Some((segment, info)) = elf.tables()? else {
        say!("no vendor tables, so nothing to read");
        return Ok(());
    };
    let tables = selfish_elf::dynamic::relocations(segment, &info);

    // Every PLT slot should name an imported function, and the count is the check: a slot
    // whose symbol is not an import is one whose address a loader has nothing to write to.
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
    // The imports a PLT slot does not cover should be data rather than functions, reached by
    // an ordinary relocation instead. Anything in neither table is an import nothing asks
    // for, which would mean the symbol filter is wrong.
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

fn title(path: &Path, round_trip: bool) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;

    // By content rather than by extension. `PARAM.SFO` inside a package is an entry rather
    // than a file, so a caller who has only the package has nothing to point an extension at.
    if let Ok(package) = selfish_pkg::Package::parse(&bytes) {
        let entry = package
            .entry(selfish_pkg::entry_id::PARAM_SFO)
            .ok_or("the package carries no PARAM.SFO entry")?;
        let raw = package
            .entry_bytes(entry)
            .ok_or("the PARAM.SFO entry runs past the end of the package")?;
        say!(
            "source     package entry {:#x}",
            selfish_pkg::entry_id::PARAM_SFO
        );
        show_sfo(&selfish_title::Sfo::parse(raw)?, round_trip.then_some(raw));
        return Ok(());
    }
    if let Ok(sfo) = selfish_title::Sfo::parse(&bytes) {
        say!("source     PARAM.SFO");
        show_sfo(&sfo, round_trip.then_some(bytes.as_slice()));
        return Ok(());
    }

    let param = selfish_title::Param::parse(&bytes)?;
    say!("source     param.json");
    say!("title id   {}", param.title_id().unwrap_or("<absent>"));
    say!("content id {}", param.content_id().unwrap_or("<absent>"));
    say!("name       {}", param.title_name().unwrap_or("<absent>"));
    say!(
        "language   {} ({} localised)",
        param.default_language().unwrap_or("<absent>"),
        param.languages().len()
    );
    if let Some(category) = param.category() {
        say!("category   {category}");
    } else {
        say!("category   <absent>");
    }
    Ok(())
}

fn show_sfo(sfo: &selfish_title::Sfo, original: Option<&[u8]>) {
    say!("entries    {}", sfo.entries().len());
    for entry in sfo.entries() {
        let value = match &entry.value {
            selfish_title::sfo::Value::Text(text) => text.clone(),
            selfish_title::sfo::Value::TextUnterminated(text) => format!("{text} (unterminated)"),
            selfish_title::sfo::Value::Integer(number) => format!("{number} ({number:#x})"),
            selfish_title::sfo::Value::Unknown(code, bytes) => {
                format!("<format {code:#06x}, {} bytes>", bytes.len())
            }
        };
        say!("  {:<20} {value}", entry.key);
    }

    // Principle 4 against real material: what was parsed, written back, has to be the same
    // bytes. Anything less means the layout is understood well enough to read and not well
    // enough to produce.
    if let Some(original) = original {
        let written = sfo.to_bytes();
        if written == original {
            say!("round trip identical, {} bytes", written.len());
        } else {
            let at = written
                .iter()
                .zip(original)
                .position(|(left, right)| left != right);
            say!(
                "round trip DIFFERS: wrote {} bytes against {}, first difference at {:?}",
                written.len(),
                original.len(),
                at
            );
        }
    }
}

/// Check a real container against the format table.
fn audit_cmd(file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(file)?;
    let result = match selfish_container::audit(&bytes) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("not a container this crate recognises: {error:?}");
            std::process::exit(1);
        }
    };

    say!("generation  {:?} (from the magic)", result.generation);
    say!(
        "confirmed   {} of {} fixed header row(s) match the table",
        result.confirmed(),
        result.header.len()
    );
    say!("");

    for row in &result.header {
        let mark = if row.matched { "  ok  " } else { " DIFF " };
        if let Some(found) = row.found {
            say!(
                "{mark} {:<14} @{:#04x}  table {:#x}  file {:#x}",
                row.field,
                row.offset,
                row.expected,
                found
            );
        } else {
            say!(
                "{mark} {:<14} @{:#04x}  table {:#x}  file (past the dump)",
                row.field,
                row.offset,
                row.expected
            );
        }
    }

    let differing = result.differing();
    if differing.is_empty() {
        say!("");
        say!("every fixed header row this file carries agrees with the table.");
    } else {
        say!("");
        say!(
            "{} row(s) differ from the previous-generation table.",
            differing.len()
        );
        say!("a difference is a finding, not a new fact: record it against a citable");
        say!("source for this generation, do not read a meaning off these bytes.");
        for row in differing {
            if !row.note.is_empty() {
                say!("  {} - the table's note: {}", row.field, row.note);
            }
        }
    }
    Ok(())
}

/// The category a native homebrew entry declares.
///
/// `0x10000`. Measured from a shipping native homebrew entry rather than chosen - the value a
/// package carries is a different number in a different field, and the two are not related.
const NATIVE_CATEGORY: i64 = 0x10000;

/// The language a title falls back to when it declares only one.
const DEFAULT_LANGUAGE: &str = "en-US";

/// Lay out a native title directory.
#[allow(
    clippy::too_many_arguments,
    reason = "a command-line command takes what the command line offers"
)]
fn native(
    out: &Path,
    title_id: &str,
    title: &str,
    content_id: Option<&str>,
    version: Option<&str>,
    deeplink: Option<&str>,
    icon: Option<&Path>,
    root: Option<&Path>,
    category: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let base = out.join(title_id);
    let sce_sys = base.join("sce_sys");
    std::fs::create_dir_all(&sce_sys)?;

    // Anything the caller wants alongside the metadata, copied verbatim. A payload usually
    // wants nothing here; a title that carries data files wants them.
    if let Some(root) = root {
        let copied = copy_tree(root, &base)?;
        say!("{copied} file(s) copied from {}", root.display());
    }

    let mut param = selfish_title::Param::new();
    param.set_native_ps5(
        title_id,
        title,
        DEFAULT_LANGUAGE,
        category,
        content_id,
        deeplink,
    );
    if let Some(ver) = version {
        param.set_version(ver);
        param.set_master_version(ver);
        say!("version: {ver}");
    }
    if let Some(cid) = content_id {
        say!("contentId: {cid}");
    }
    if let Some(uri) = deeplink {
        say!("deeplinkUri: {uri}");
    }
    let param_path = sce_sys.join("param.json");
    std::fs::write(&param_path, param.to_bytes()?)?;
    say!("{}", param_path.display());

    let icon_path = sce_sys.join("icon0.png");
    if let Some(path) = icon {
        std::fs::copy(path, &icon_path)?;
        say!("{} (from {})", icon_path.display(), path.display());
    } else {
        std::fs::write(&icon_path, icon::default_icon()?)?;
        say!("{} (generated)", icon_path.display());
    }

    say!("");
    say!("install by copying {} to /user/app/ on the", base.display());
    say!("target and calling sceAppInstUtilAppInstallTitleDir(\"{title_id}\", \"/user/app/\", 0).");
    say!("that call needs kernel privileges, so it runs from a payload rather than from here.");
    Ok(())
}

/// Copy a directory tree, returning how many files were written.
fn copy_tree(from: &Path, to: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    let mut count = 0_usize;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&target)?;
            count = count.saturating_add(copy_tree(&entry.path(), &target)?);
        } else {
            std::fs::copy(entry.path(), &target)?;
            count = count.saturating_add(1);
        }
    }
    Ok(count)
}

/// Build a filesystem image from a directory, and say what went into it.
fn image_cmd(
    root: &Path,
    out: &Path,
    content_id: &str,
    passcode: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let passcode: Vec<u8> = passcode.map_or_else(
        || selfish_pkg::keys::FAKE_PASSCODE.to_vec(),
        |text| text.as_bytes().to_vec(),
    );
    let tree = read_tree(root)?;
    let image = build_image(&tree, content_id, &passcode)?;
    std::fs::write(out, &image)?;
    say!("{}: {} bytes", out.display(), image.len());
    say!("keyed to {content_id} - a package carrying this must declare the same id");
    Ok(())
}

/// Read a directory into a tree the filesystem writer understands.
///
/// Sorted, because a package built twice from the same directory should be the same package -
/// and a filesystem's directory order otherwise leaks into the output.
fn read_tree(root: &Path) -> Result<selfish_pfs::write::Tree, Box<dyn std::error::Error>> {
    fn walk(at: &Path, name: &str) -> Result<selfish_pfs::write::Tree, Box<dyn std::error::Error>> {
        let mut tree = selfish_pfs::write::Tree::new(name);
        let mut entries: Vec<_> = std::fs::read_dir(at)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type()?.is_dir() {
                tree = tree.with_dir(walk(&path, &name)?);
            } else {
                tree = tree.with_file(&name, std::fs::read(&path)?);
            }
        }
        Ok(tree)
    }
    walk(root, selfish_pfs::write::ROOT_NAME)
}

/// Build the image a package carries, from a tree.
///
/// Three layers, and the keys for the outer one come from the content id and the passcode.
/// How large the *inner* filesystem inside an outer image is, or `None` if it cannot be read.
///
/// The package header's cache size has to be compared against this rather than against the outer
/// image, which is larger and would have hidden the problem: a minimal package's outer image was
/// comfortably above the declared cache while its inner filesystem was below it, and the console
/// refused the mount.
///
/// Reaching it means decrypting, because the inner image is a `PFSC` container held as a file
/// inside the encrypted outer filesystem. Everything needed is in hand - the key comes from the
/// content id and the passcode - so this is the same walk [`open`] does, from an image rather than
/// from a whole package.
///
/// Returns `None` rather than failing the build: a package whose image cannot be walked has a
/// larger problem than its cache size, and it will be reported by whatever reads it next.
fn inner_image_size(image: &[u8], content_id: &str, passcode: &[u8]) -> Option<u64> {
    use selfish_pfs::{Filesystem, Slice, Source, Xts};

    let ekpfs = selfish_pkg::keys::derive_filesystem_key(content_id.as_bytes(), passcode);
    let source = Slice::new(image, 0);
    // The superblock is in the clear even where the rest is not, which is what carries the seed.
    let superblock = source.read(0, 0x400).ok()?;
    let block_size = u64::from(u32::from_le_bytes([
        *superblock.get(0x20)?,
        *superblock.get(0x21)?,
        *superblock.get(0x22)?,
        *superblock.get(0x23)?,
    ]));
    let (tweak, data) = selfish_pfs::image_keys(&ekpfs, &superblock).ok()?;
    let sectors = block_size.checked_div(selfish_pfs::SECTOR_SIZE)?;
    let decrypted = Xts::new(source, &tweak, &data, sectors).ok()?;
    let outer = Filesystem::new(&decrypted).ok()?;
    for found in outer.walk(0).ok()? {
        if !found.path.ends_with(selfish_pfs::outer::IMAGE_NAME) {
            continue;
        }
        // Only the `PFSC` header is needed: it records the length its contents decompress to.
        let contents = outer.contents(found.inode).ok()?;
        let raw = contents.get(0x28..0x30)?;
        let mut value = [0_u8; 8];
        value.copy_from_slice(raw);
        return Some(u64::from_le_bytes(value));
    }
    None
}

/// Whether a tree already carries a keystone.
fn has_keystone(tree: &selfish_pfs::write::Tree) -> bool {
    tree.dirs
        .iter()
        .filter(|dir| dir.name == SCE_SYS)
        .any(|dir| dir.files.iter().any(|(name, _)| name == "keystone"))
}

/// Put a file in , creating the directory if the tree has none.
fn add_to_sce_sys(tree: &mut selfish_pfs::write::Tree, name: &str, bytes: Vec<u8>) {
    if let Some(dir) = tree.dirs.iter_mut().find(|dir| dir.name == SCE_SYS) {
        dir.files.push((name.to_owned(), bytes));
    } else {
        tree.dirs
            .push(selfish_pfs::write::Tree::new(SCE_SYS).with_file(name, bytes));
    }
}

/// The directory a title own metadata lives in.
const SCE_SYS: &str = "sce_sys";

fn build_image(
    tree: &selfish_pfs::write::Tree,
    content_id: &str,
    passcode: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    const BLOCK: u32 = 0x10000;

    // Every real package carries `sce_sys/keystone`, and it is derived from the passcode rather
    // than supplied - so it is put in here rather than demanded from a caller who has no way to
    // compute it. A caller that provided its own is left alone.
    let mut owned = tree.clone();
    if has_keystone(&owned) {
        say!("sce_sys/keystone: supplied by the caller, left alone");
    } else {
        add_to_sce_sys(
            &mut owned,
            "keystone",
            selfish_pkg::keystone::create(passcode)?,
        );
        say!("sce_sys/keystone: generated from the passcode");
    }
    let tree = &owned;

    let inner = selfish_pfs::write::build(tree, BLOCK)?;
    let container = selfish_pfs::pfsc::wrap(&inner, BLOCK)?;
    let ekpfs = selfish_pkg::keys::derive_filesystem_key(content_id.as_bytes(), passcode);
    Ok(selfish_pfs::outer::build(&selfish_pfs::outer::Options {
        payload: &container,
        ekpfs: &ekpfs,
        seed: [0; 16],
        encrypt: true,
        block_size: BLOCK,
    })?)
}

/// The icon entry, which this crate does not name because it cannot identify it from evidence.
const ICON_ENTRY: u32 = 0x1200;
/// The playgo manifest entry.
const PLAYGO_MANIFEST_ENTRY: u32 = 0x1003;
/// The playgo manifest a single-chunk title carries.
///
/// The earlier string here (`version="1000"`, `<available>1</available>`) was **not** what a real
/// package holds and was refused: `scePlayGoCoreGetRawContentInfo` reads this to learn the
/// content's chunk and scenario layout, and given a manifest with neither it returned
/// `0x80f00200`. This is the structure a real package carries - one chunk, one scenario - opened
/// with a UTF-8 BOM as the sample is. (measured against a real package; an oracle, not a source)
const PLAYGO_MANIFEST: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\n<psproject fmt=\"playgo-manifest\" version=\"0990\">\n  <volume>\n    <chunk_info chunk_count=\"1\" scenario_count=\"1\">\n      <scenarios default_id=\"0\">\n        <scenario id=\"0\" type=\"sp\" initial_chunk_count=\"1\" label=\"Scenario #0\">0</scenario>\n      </scenarios>\n    </chunk_info>\n  </volume>\n</psproject>\n";

/// Which entry ids the caller supplied on the command line.
fn builder_ids(entries: &[String]) -> Vec<u32> {
    entries
        .iter()
        .filter_map(|spec| spec.split_once('=').map(|(id, _)| id))
        .filter_map(|id| {
            id.strip_prefix("0x")
                .map_or_else(|| id.parse().ok(), |hex| u32::from_str_radix(hex, 16).ok())
        })
        .collect()
}

/// The title id is the middle field of a content id.
fn middle_of(content_id: &str) -> &str {
    content_id
        .split('-')
        .nth(1)
        .and_then(|part| part.split('_').next())
        .unwrap_or(content_id)
}

#[allow(
    clippy::too_many_arguments,
    reason = "a command-line command takes what the command line offers"
)]
#[allow(
    clippy::too_many_lines,
    reason = "one linear assembly: read the image, warn about what the hardware will refuse, take \
              the supplied entries, then fill in each default and say which it filled. Splitting \
              it would scatter the order the defaults are decided in, and that order is what the \
              output reports"
)]
fn pack(
    image: Option<&Path>,
    dir: Option<&Path>,
    passcode: Option<&str>,
    out: &Path,
    content_id: &str,
    entries: &[String],
    title_id: Option<&str>,
    title: Option<&str>,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let passcode: Vec<u8> = passcode.map_or_else(
        || selfish_pkg::keys::FAKE_PASSCODE.to_vec(),
        |text| text.as_bytes().to_vec(),
    );

    let image = match (image, dir) {
        (Some(path), _) => std::fs::read(path)?,
        (None, Some(root)) => {
            let tree = read_tree(root)?;
            let image = build_image(&tree, content_id, &passcode)?;
            say!("image: {} bytes from {}", image.len(), root.display());
            image
        }
        (None, None) => return Err("one of --image or --dir is required".into()),
    };

    // A small inner filesystem cannot be mounted, and nothing this crate writes can change that.
    //
    // A console refuses one after the outer image has already mounted and `pfs_image.dat` has
    // already opened: `sceFsMountGamePkg ***ERR*** Failed to enable GDDR5 cache`, `EINVAL`. The
    // obvious reading is that the header's declared cache size is too large for the image, and it
    // is wrong - built with the declared size set to exactly the inner size, a console refuses it
    // identically, at the same line. **The inner filesystem itself has to be big enough**, and the
    // threshold is somewhere in `(720896, 1769472]`. (D071, correcting D070)
    //
    // So this warns rather than adjusting anything. The warning says lowering the declared size
    // does not help, because that is the next thing anyone who hits this will try.
    let builder = selfish_pkg::write::Builder::new()
        .content_id(content_id)
        .passcode(&passcode);
    if let Some(inner) = inner_image_size(&image, content_id, &passcode)
        .filter(|inner| *inner < u64::from(selfish_pkg::write::DEFAULT_CACHE_SIZE))
    {
        say!(
            "warning: the inner filesystem is {inner} bytes. The hardware refuses to mount an image \
             this small - `Failed to enable GDDR5 cache`, EINVAL, after the outer image has \
             already mounted - and lowering the declared cache size does NOT help: it was tried, \
             set to exactly {inner}, and the hardware refused it identically. Pad the directory \
             past {} bytes.",
            selfish_pkg::write::DEFAULT_CACHE_SIZE
        );
    }
    let mut builder = builder.image(image);

    for spec in entries {
        let (id, path) = spec
            .split_once('=')
            .ok_or_else(|| format!("--entry wants ID=FILE, got {spec:?}"))?;
        let id = id.strip_prefix("0x").map_or_else(
            || {
                id.parse::<u32>()
                    .map_err(|_| format!("bad entry id {id:?}"))
            },
            |hex| u32::from_str_radix(hex, 16).map_err(|_| format!("bad entry id {id:?}")),
        )?;
        let bytes = std::fs::read(path)?;
        // The icon is the one supplied entry with a requirement a caller cannot see: a console
        // wants it 512x512 with no alpha, and an icon that is neither is accepted and then drawn
        // wrongly rather than refused. Converting it here means a caller supplies the picture they
        // have and gets the picture a console wants. (D073)
        let bytes = if id == ICON_ENTRY {
            let converted = icon::normalise(&bytes, path)?;
            if converted.len() == bytes.len() {
                say!("icon0.png: {path}");
            } else {
                say!("icon0.png: {path}, converted to 512x512 RGB for the hardware");
            }
            converted
        } else {
            bytes
        };
        builder = builder.entry(id, bytes);
    }

    // Stand-ins for two entries a package must carry, supplied only when the caller did not.
    //
    // The line between them is the one this repository draws everywhere else. A `param.sfo` is
    // a **format** - `selfish_pkg::sfo` writes a real one, and what was being shipped in its
    // place was twenty-one bytes of the word PLACEHOLDER, which a console rejects on the magic.
    // An **icon is a picture**: there is no format here to be right about, so what is emitted
    // is a valid but plainly blank PNG rather than any particular image, and certainly not one
    // carrying this project's identity into somebody else's package.
    //
    // Both are announced. A default nobody was told about is worse than a missing file.
    let supplied: Vec<u32> = builder_ids(entries);
    if !supplied.contains(&selfish_pkg::entry_id::PARAM_SFO) {
        let title_id = title_id.unwrap_or_else(|| middle_of(content_id));
        let title = title.unwrap_or(title_id);
        say!("param.sfo: generated for {title_id} ({title:?}, version {version})");
        builder = builder.entry(
            selfish_pkg::entry_id::PARAM_SFO,
            selfish_pkg::sfo::game_bytes(&selfish_pkg::sfo::Params {
                content_id,
                title_id,
                title,
                version,
            }),
        );
    }
    if !supplied.contains(&ICON_ENTRY) {
        say!("icon0.png: none given, using selfish own - supply --entry 0x1200=FILE to replace");
        builder = builder.entry(ICON_ENTRY, icon::default_icon()?);
    }
    if !supplied.contains(&PLAYGO_MANIFEST_ENTRY) {
        say!("playgo-manifest.xml: generated the default manifest");
        builder = builder.entry(PLAYGO_MANIFEST_ENTRY, PLAYGO_MANIFEST.as_bytes().to_vec());
    }

    // Printed rather than returned: the top level renders an error with `Debug`, which would
    // turn a carefully worded list of missing entries into decimal integers.
    let built = match builder.build() {
        Ok(built) => built,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    std::fs::write(out, &built.bytes)?;
    say!(
        "{}: {} bytes, {} entries, image at {:#x}",
        out.display(),
        built.bytes.len(),
        built.entries,
        built.image_at
    );

    // Loud rather than quiet. A package with holes in it installs or does not, and finding out
    // from a console tells you nothing about which byte was wrong.
    if built.is_complete() {
        say!("no gaps: every byte written is one this crate can account for");
    } else {
        say!(
            "{} gap(s) left blank, because nothing established says what goes in them:",
            built.gaps.len()
        );
        for gap in &built.gaps {
            say!(
                "  entry {:#x} at {:#x}, {} bytes - {}",
                gap.entry,
                gap.offset,
                gap.length,
                gap.what
            );
        }
    }
    Ok(())
}

fn derive(paths: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    if paths.is_empty() {
        return Err("give it some packages: selfish derive a.pkg b.pkg ...".into());
    }

    // Read them all first, because the borrow has to outlive the parse.
    let mut bytes = Vec::new();
    for path in paths {
        bytes.push((path.clone(), std::fs::read(path)?));
    }
    let mut packages = Vec::new();
    for (path, raw) in &bytes {
        match selfish_pkg::Package::parse(raw) {
            Ok(package) => packages.push(package),
            Err(error) => say!("skipped {}: {error}", path.display()),
        }
    }

    let derivation = selfish_pkg::derive::run(&packages);
    say!("samples    {}", derivation.samples);
    if derivation.samples < 2 {
        say!("  a single package cannot distinguish a format from a coincidence");
    }
    say!();

    for finding in &derivation.findings {
        say!(
            "entry {:#06x}  {}/{} packages agree{}",
            finding.entry,
            finding.held,
            finding.tested,
            if finding.survived() {
                ""
            } else {
                "  <-- FAILED"
            }
        );
        say!("  claim    {}", finding.claim);
        for note in &finding.notes {
            say!("  note     {note}");
        }
    }

    say!();
    if derivation.is_consistent() {
        say!("every claim survived every package it could be tested on");
        Ok(())
    } else {
        say!("a claim failed: the table in data/pkg-format.tsv is wrong, or this package is");
        say!("built by something that disagrees. Either is worth knowing.");
        Err("derivation inconsistent".into())
    }
}

fn container(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let container = selfish_container::Container::parse(&bytes)?;
    say!("generation {}", container.generation());
    say!("entries    {}", container.entries().len());
    say!(
        "header     {:#x}, metadata {:#x}, stated size {:#x}",
        container.header_size(),
        container.meta_size(),
        container.stated_file_size()
    );
    for entry in container.entries() {
        say!(
            "  {}  segment {:>3}  {:#012x} {:>10} bytes",
            if entry.carries_segment_data() {
                "data  "
            } else {
                "digest"
            },
            entry.segment_index(),
            entry.offset,
            entry.filesz
        );
    }
    match container.inner_elf_header() {
        Ok(_) => say!("inner executable at {:#x}", container.inner_offset()),
        Err(error) => say!("inner executable: {error}"),
    }
    Ok(())
}

fn wrap(path: &Path, out: Option<&Path>, generation: u8) -> Result<(), Box<dyn std::error::Error>> {
    let generation = Generation::from_number(generation)
        .ok_or("generation must be 4 or 5; anything else is a typo rather than a generation")?;
    let payload = std::fs::read(path)?;
    let container = selfish_container::build(&payload, generation)?;
    let target = out.map_or_else(|| path.with_file_name("eboot.bin"), PathBuf::from);
    std::fs::write(&target, &container)?;
    say!(
        "{}: {} bytes from a {} byte payload, {generation}",
        target.display(),
        container.len(),
        payload.len()
    );
    Ok(())
}

/// The decryption layer over a package's image, with what its superblock said.
///
/// Named rather than returned as a tuple: three values whose meanings are not obvious from
/// their types, which is exactly when a tuple stops being convenient.
struct Image<'a> {
    decrypted: Box<Xts<Slice<'a>>>,
    block_size: u64,
    /// Where the image begins. Carried rather than re-derived, so a caller reporting on a
    /// package does not parse the header a second time to say where it looked.
    offset: u64,
}

/// Everything between a package and its files, in one place.
///
/// Returned rather than printed so both `pkg` and `extract` walk the same path - two
/// implementations of this nesting would be two chances to get it subtly different.
struct Opened<'a> {
    files: Vec<selfish_pfs::Found>,
    inner: Filesystem<Compressed<Region<&'a Xts<Slice<'a>>>>>,
}

fn open(bytes: &[u8]) -> Result<Image<'_>, Box<dyn std::error::Error>> {
    let package = selfish_pkg::Package::parse(bytes)?;
    let key = selfish_pkg::keys::filesystem_key(&package)?;
    let at = package.image_offset()?;

    // The superblock is in the clear even where the rest is not, which is what makes the key
    // derivation possible at all: it carries the seed.
    let image = Slice::new(bytes, at);
    let superblock = image.read(0, 0x400)?;
    let block_size = u64::from(u32::from_le_bytes([
        *superblock.get(0x20).unwrap_or(&0),
        *superblock.get(0x21).unwrap_or(&0),
        *superblock.get(0x22).unwrap_or(&0),
        *superblock.get(0x23).unwrap_or(&0),
    ]));
    if block_size == 0 {
        return Err("the image superblock declares a zero block size".into());
    }
    let (tweak, data) = selfish_pfs::image_keys(&key, &superblock)?;
    let sectors = block_size
        .checked_div(selfish_pfs::SECTOR_SIZE)
        .unwrap_or(0);
    Ok(Image {
        decrypted: Box::new(Xts::new(image, &tweak, &data, sectors)?),
        block_size,
        offset: at,
    })
}

fn walk<'a>(decrypted: &'a Xts<Slice<'a>>) -> Result<Opened<'a>, Box<dyn std::error::Error>> {
    let outer = Filesystem::new(decrypted)?;
    // The inner image is the largest file in the outer filesystem. Chosen that way rather
    // than by a fixed block number, because a previous-generation extractor hardcodes block
    // eleven and these packages put it at seven.
    let biggest = outer
        .inodes()
        .iter()
        .max_by_key(|inode| inode.size)
        .copied()
        .ok_or("the outer filesystem is empty")?;
    let window = Region::new(
        // Dereferenced once: the filesystem was built from a reference, so its `source()` is
        // a reference to a reference. The window wants the layer itself.
        *outer.source(),
        u64::from(biggest.start).saturating_mul(outer.block_size()),
        biggest.size,
    );
    let inner = Filesystem::new(Compressed::new(window)?)?;
    let files = inner.walk(0)?;
    Ok(Opened { files, inner })
}

fn pkg(path: &Path, all: bool) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let package = selfish_pkg::Package::parse(&bytes)?;
    say!("entries    {}", package.entries().len());
    let missing = package.missing_expected_entries();
    if missing.is_empty() {
        say!("           every expected entry present");
    } else {
        say!("           MISSING {missing:#x?}");
    }

    let image = open(&bytes)?;
    say!("image at   {:#x}", image.offset);
    say!("block size {:#x}", image.block_size);
    let opened = walk(&image.decrypted)?;
    say!("files      {}", opened.files.len());

    let shown = if all { opened.files.len() } else { 40 };
    for found in opened.files.iter().take(shown) {
        let size = opened.inner.inodes().get(found.inode).map_or(0, |i| i.size);
        say!("  {size:>12}  {}", found.path);
    }
    if opened.files.len() > shown {
        say!(
            "  ... and {} more",
            opened.files.len().saturating_sub(shown)
        );
    }
    Ok(())
}

fn extract(path: &Path, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let image = open(&bytes)?;
    let opened = walk(&image.decrypted)?;

    let mut written = 0_usize;
    for found in &opened.files {
        // A path out of a file is untrusted input. Anything that would climb out of the
        // destination is refused rather than sanitised, because a filesystem describing
        // `../..` is a finding and not something to quietly correct.
        if found.path.contains("..") {
            eprintln!(
                "refusing {}: the path would escape the destination",
                found.path
            );
            continue;
        }
        let relative = found.path.trim_start_matches('/');
        let target = out.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = opened.inner.contents(found.inode)?;
        std::fs::write(&target, &contents)?;
        written = written.saturating_add(1);
    }
    say!("{written} files written to {}", out.display());
    Ok(())
}
