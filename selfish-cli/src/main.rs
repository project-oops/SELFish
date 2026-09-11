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
use std::sync::OnceLock;

use clap::{Parser, Subcommand};
use selfish_abi::Generation;
use selfish_pfs::{Compressed, Filesystem, Region, Slice, Source, Xts};

/// This build in one line, for clap's `--version`: `v{version} - {commit}`, the commit stamped by
/// `build.rs`.
///
/// A plain function rather than the shared `oops_build::line!` macro it replaced: `env!` and
/// `option_env!` read the crate they expand in, so expanding them *here* is what makes them report
/// this crate's own version and the `OOPS_COMMIT` its own build script set - a macro in another
/// crate would report that crate's. Computed once and kept because clap wants a `&'static str` and
/// the commit is only knowable after `build.rs` has run. A build made outside a repository has no
/// commit to name and says so. (D104)
fn version_line() -> &'static str {
    static LINE: OnceLock<String> = OnceLock::new();
    LINE.get_or_init(|| {
        let version = env!("CARGO_PKG_VERSION");
        match option_env!("OOPS_COMMIT").filter(|commit| !commit.is_empty()) {
            Some(commit) => format!("v{version} - {commit}"),
            None => format!("v{version} - no commit"),
        }
    })
    .as_str()
}

#[derive(Parser)]
#[command(
    name = "selfish",
    version = version_line(),
    about = "Read and write the file formats Prospero-generation hardware loads"
)]
struct Cli {
    /// The executable to build from. Begins a pipeline invocation.
    ///
    /// `--input`, `--target`, `--format` and `--output` are one pipeline run and are required
    /// together. With none of them, `selfish` takes a diagnostic subcommand instead.
    #[arg(long, value_name = "FILE", requires_all = ["target", "format", "output"])]
    input: Option<PathBuf>,

    /// Which machine the artifact is for. Replaces `--generation`.
    #[arg(long, value_enum, requires = "input")]
    target: Option<Target>,

    /// What to produce.
    #[arg(long, value_enum, requires = "input")]
    format: Option<Format>,

    /// Where to write it. Written exactly, with nothing placed beside it.
    #[arg(long, value_name = "PATH", requires = "input")]
    output: Option<PathBuf>,

    /// What the title declares itself to be. `--format title` and `--format pkg` only.
    ///
    /// Defaults to `system-app`, which is what this tool has always produced for homebrew.
    #[arg(long, value_enum, requires = "input")]
    category: Option<Category>,

    /// Privilege tier the container declares: app, sysmodule, system, or root.
    ///
    /// Not meaningful for `--format elf` or `--format prx`, which produce no container, and
    /// passing it with either is an error rather than a no-op.
    ///
    /// `sysmodule` currently writes exactly what `app` does - the container comes out
    /// byte-identical - so it is accepted and changes nothing.
    ///
    /// `system` also sets the title's category to `0x20000` - a pairing carried over from
    /// `title_dir`, not one measured here. Nothing has checked what a loader does with a
    /// system-tier container under any other category.
    #[arg(long, value_name = "TIER", requires = "input")]
    privilege: Option<String>,

    /// Target SDK version or alias to pin, such as `2.000.009`, `prospero` or `orbis`.
    ///
    /// Patches `PT_SCE_PROCPARAM` and the container's declared version. Same formats as
    /// `--privilege`, and the same refusal for `elf` and `prx`.
    #[arg(long, value_name = "VERSION", requires = "input")]
    sdk: Option<String>,

    /// A `sdk-versions.toml` to read instead of the embedded one.
    #[arg(long, value_name = "FILE", requires = "sdk")]
    sdk_table: Option<PathBuf>,

    /// The title id an artifact carries. Optional; defaults to `OBSC00001`, and says so.
    ///
    /// **It cannot be read out of `--input`, and this tool will not invent a place to put it.**
    /// There is no title id in an ELF: `PT_SCE_PROCPARAM` is the only per-process block the
    /// platform reads before anything runs and it holds SDK versions (`ORBI` magic, versions at
    /// `+0x10` and `+0x14`) and no identity; `PT_SCE_MODULE_PARAM` is the same; and the ELF
    /// specification has no home for one either.
    ///
    /// A note section or an agreed magic string would work and would be this collection's own
    /// invented standard - enforced nowhere, understood by nothing that ships, and a dependency
    /// on a generator rather than on a format. So it stays an option with a stated default,
    /// which is honest about a fact that is genuinely not present in the input.
    ///
    /// Where a title id does live, all of it *title metadata* rather than executable content -
    /// which is the point, since the id is a property of a title and a bare executable is not
    /// one yet: `param.json` → `titleId`; `PARAM.SFO` → `TITLE_ID`; and a package's content id,
    /// `UP0000-PPSA01650_00-…`, between the first `-` and the `_`.
    #[arg(long, requires = "input")]
    title_id: Option<String>,

    /// The name shown on the home screen. Defaults to the title id.
    #[arg(long, requires = "input")]
    title: Option<String>,

    /// The content id, such as `UP0000-PPSA01650_00-YOUTUBE000000000`. `--format title` and
    /// `--format pkg` only; refused elsewhere.
    ///
    /// For `pkg` it is not cosmetic: the filesystem image is keyed by a hash of the content id,
    /// so a package built with the wrong one produces an image a console cannot open. It also
    /// lands in `param.json` / `param.sfo`. Defaults to an id derived from the title id.
    #[arg(long, value_name = "ID", requires = "input")]
    content_id: Option<String>,

    /// The title version, as `NN.NN`. `--format title` and `--format pkg` only.
    ///
    /// Sets `param.json` / `param.sfo`'s version and master version. Defaults to `01.00`.
    /// Spelled `--title-version` because `--version` is the tool's own build-version flag.
    #[arg(long = "title-version", value_name = "NN.NN", requires = "input")]
    title_version: Option<String>,

    /// A 512x512 PNG for the home-screen tile. `--format title` and `--format pkg` only.
    ///
    /// Flattened to the no-alpha RGB a console wants - an icon that keeps its transparency is
    /// composited wrongly and reads square on a home screen. It is **not resized**: a non-512x512
    /// PNG is refused rather than scaled, because which filter to use is a decision about the
    /// artwork. For `pkg` it fills both the mounted tile and the `0x1200` store tile. Without one,
    /// a default mark is used that says selfish built this and nobody supplied artwork. (D073)
    #[arg(long, value_name = "FILE", requires = "input")]
    icon: Option<PathBuf>,

    /// Make the entry launch a URI instead of carrying its own executable. `--format title` and
    /// `--format pkg` only.
    ///
    /// A launcher tile - the shape to use when the code is already running as a payload and the
    /// home-screen entry only has to reach it. Sets `param.json`'s deeplink.
    #[arg(long, value_name = "URI", requires = "input")]
    deeplink: Option<String>,

    /// A package entry, as `ID=FILE`. `--format pkg` only.
    ///
    /// Overrides an entry this crate would otherwise compute or generate - for rebuilding a
    /// package to match existing material. Nothing is required: `0x200` and `0x1001` are
    /// computed now (D099), and the icon has its own `--icon`.
    #[arg(long = "entry", value_name = "ID=FILE", requires = "input")]
    entries: Vec<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

/// Category identifier for default Prospero homebrew entries (System App / 0x10000).
/// Which machine an artifact is for.
///
/// **This is where the generation lives now.** `--generation 4|5` is gone from the command
/// line: `orbis` and `neo` are the previous generation, `prospero` and `trinity` the current
/// one, so the generation is derivable and stops being a thing anyone passes.
///
/// `neo` and `trinity` are the **mid-generation refreshes and are not synonyms** for `orbis`
/// and `prospero`. An artifact for any previous-generation machine is `orbis`; `neo` is correct
/// only for a genuinely Pro-specific build. Artifact names and release URLs follow this.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "lower")]
enum Target {
    /// The previous generation's base machine.
    Orbis,
    /// The previous generation's mid-cycle refresh. Not a synonym for `orbis`.
    Neo,
    /// The current generation's base machine.
    Prospero,
    /// The current generation's mid-cycle refresh. Not a synonym for `prospero`.
    Trinity,
}

impl Target {
    /// The container generation this machine loads.
    const fn generation(self) -> Generation {
        match self {
            Self::Orbis | Self::Neo => Generation::Previous,
            Self::Prospero | Self::Trinity => Generation::Current,
        }
    }
}

/// What to produce.
///
/// Each is a pure function of `--input` and the other options: it writes exactly `--output`
/// and nothing beside it, and needs no directory the caller has to prepare or clean.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "lower")]
enum Format {
    /// The executable, stamped with the target's platform identity. No container.
    Elf,
    /// A shared library, stamped as one. No container.
    ///
    /// The same stamping `elf` does with a different `e_type`, which is the whole difference
    /// between an executable and a `.prx` at this layer. It is a `--format` rather than a flag
    /// because what comes out is a different kind of file, which is what this axis selects.
    Prx,
    /// A signed-executable container - an `eboot.bin`.
    Eboot,
    /// A title directory, laid out as `<output>/<TITLE_ID>/` - the shape the install call takes.
    Title,
    /// An installable package.
    Pkg,
}

/// What a title declares itself to be.
///
/// A build-side lever with runtime consequences: the loader decides an artifact's memory
/// budget and whether it owns the display from this, and refuses a previous-generation
/// category the current generation's libraries. Only meaningful for `--format title` and
/// `--format pkg`; passing it with `elf` or `eboot` is an error rather than a no-op, because a
/// silently ignored option is how somebody believes they set something.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "kebab-case")]
enum Category {
    /// Full memory budget and exclusive primary display. A game.
    BigApp,
    /// No direct memory and no primary display. **The default**, and what homebrew has been
    /// getting: it needs neither `libc.prx` nor a `pltauth` patch.
    SystemApp,
    /// Constrained budget, overlay display, runs alongside a big app.
    MiniApp,
    /// Headless background service.
    Daemon,
    /// Media streaming budget and a protected video path.
    MediaApp,
}

impl Category {
    /// The value the metadata carries.
    const fn value(self) -> i64 {
        match self {
            Self::BigApp => selfish_title::category::BIG_APP,
            Self::SystemApp => selfish_title::category::SYSTEM_APP,
            Self::MiniApp => selfish_title::category::MINI_APP,
            Self::Daemon => selfish_title::category::DAEMON,
            Self::MediaApp => selfish_title::category::MEDIA_APP,
        }
    }
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
    /// Build an AGC shader container - the header `sceAgcCreateShader` is handed.
    ///
    /// The container *format* is this tool's (from `data/agc-shader-format.tsv`, D103); the
    /// register *contents* are the shader's, read from its bytecode by whoever produced it and
    /// passed in with `--sh-reg`. The bytecode itself is not embedded - a console fills the code
    /// pointer at create time - so this writes the header and its sub-tables, and records the
    /// bytecode's size.
    ///
    /// `--stage` names the shader stage; `compute`, `pixel` and `vertex` have citable type values,
    /// and any other stage takes a raw type value the caller has confirmed itself.
    Shader {
        /// The shader stage: `compute` (default), `pixel`, `vertex`, or a raw `type` value (hex or
        /// decimal) for a stage without a citable constant here.
        #[arg(long, default_value = "compute")]
        stage: String,
        /// The compiled shader bytecode. Its length is recorded as `shader_size`; the bytes are
        /// not embedded. Use `--shader-size` instead if you only have the size.
        #[arg(long, value_name = "FILE")]
        code: Option<PathBuf>,
        /// The bytecode size, when `--code` is not given. One of the two is required.
        #[arg(long, value_name = "BYTES")]
        shader_size: Option<u32>,
        /// The ISA target the bytecode is for. Defaults to `0x0e` (RDNA2), the one measured.
        #[arg(long, value_name = "N", default_value = "0x0e")]
        target: String,
        /// An SH register, as `OFFSET=VALUE` (hex or decimal), repeatable. A compute shader needs
        /// at least the program-address pair `0x20c=0` and `0x20d=0`, which a console patches from
        /// the code pointer; its resource registers come from the bytecode.
        #[arg(long = "sh-reg", value_name = "OFFSET=VALUE")]
        sh_registers: Vec<String>,
        /// Where to write the container.
        #[arg(long, short)]
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

/// Print the error, not its `Debug`, and exit non-zero.
///
/// Returning `Result` from `main` makes Rust print `Error: {:?}`, so every `Display` in these
/// crates - the ones that say what a reader should do about a failure - was written for nobody.
/// `SegmentNotInFile` is the case that made it obvious: its `Debug` is three numbers, and its
/// `Display` explains that an executable taken out of a container is a view. (D095)
fn main() {
    if let Err(error) = run() {
        eprintln!("selfish: {error}");
        std::process::exit(1);
    }
}

/// Dispatch, which is one arm per subcommand and long for that reason alone.
#[allow(clippy::too_many_lines)]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Two ways in, and they do not mix. Four options are a build; a subcommand is a
    // diagnostic somebody runs by hand on a file they already have.
    let Some(command) = cli.command else {
        if cli.input.is_some() {
            return pipeline(&cli);
        }
        // Neither: clap's own help, rather than a bespoke error that says less.
        <Cli as clap::CommandFactory>::command().print_help()?;
        return Ok(());
    };

    match command {
        Command::Nid { names } => nid(&names),
        Command::Elf { file } => elf(&file),
        Command::Imports { file, all } => imports(&file, all),
        Command::Sections { file, defines } => sections(&file, &defines),
        Command::Reloc { file } => reloc(&file),
        Command::Container { file } => container(&file),
        Command::Title { file, round_trip } => title(&file, round_trip),
        Command::Audit { file } => audit_cmd(&file),
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
        Command::Shader {
            stage,
            code,
            shader_size,
            target,
            sh_registers,
            out,
        } => shader(
            &stage,
            code.as_deref(),
            shader_size,
            &target,
            &sh_registers,
            &out,
        ),
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
            selfish_title::sfo::Value::Binary(bytes) => {
                format!("<{} bytes, unterminated>", bytes.len())
            }
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
/// The `ex_info` rows, reported separately from the header on purpose.
///
/// A header row differing is a claim about the container format; a tail row differing usually
/// just means "not a fake container", because these values came from a writer that made no
/// other kind. One combined count would mean neither.
fn show_tail(result: &selfish_container::Audit) {
    let Some(first) = result.tail.first() else {
        say!("");
        say!("ex_info     not reachable at `header_size - 0x70`, so the tail was not checked.");
        return;
    };

    say!("");
    say!("ex_info     the tail, at {:#x}", first.offset);
    for row in &result.tail {
        let mark = if row.matched { "  ok  " } else { " DIFF " };
        if let Some(found) = row.found {
            say!(
                "{mark} {:<14} @{:#06x}  table {:#x}  file {:#x}",
                row.field,
                row.offset,
                row.expected,
                found
            );
        } else {
            say!(
                "{mark} {:<14} @{:#06x}  table {:#x}  file (past the dump)",
                row.field,
                row.offset,
                row.expected
            );
        }
    }

    let differing = result.tail_differing();
    if !differing.is_empty() {
        say!("");
        say!(
            "{} tail row(s) differ. These are what a *fake* container carries, so this",
            differing.len()
        );
        say!("is expected of anything this project did not write - and it is not evidence");
        say!("that the layout is wrong. Read it with the `kind` line above.");
    }
}

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
    // Printed **before** the row count, deliberately. The count is the number a reader
    // remembers, and on a container written from this table it is nine out of nine no matter
    // what - so what kind of container this is has to arrive first or it arrives too late.
    // (D092: it arrived too late once, and the confirmation it produced had to be reversed.)
    match &result.declared {
        selfish_container::Declared::Ptype { value, known } => {
            let name = known.as_deref().unwrap_or("not a kind this table names");
            say!("kind        ptype {value:#x} - {name}");
        }
        selfish_container::Declared::Unreachable => {
            say!("kind        unknown - `ex_info` is not at `header_size - 0x70` in this file");
        }
    }
    if let Some(caveat) = result.declared.caveat() {
        say!("            {caveat}");
    }
    say!("");

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

    show_tail(&result);
    Ok(())
}

/// The language a title falls back to when it declares only one.
const DEFAULT_LANGUAGE: &str = "en-US";

/// The default title id, when `--title-id` is not given.
const DEFAULT_TITLE_ID: &str = "OBSC00001";

/// One pipeline invocation: `--input` through `--format` to `--output`.
///
/// # The contract this owes its caller
///
/// Every arm is a **pure function of its inputs**: it reads `--input`, writes exactly
/// `--output`, and puts nothing beside it. Nothing here needs a staging directory the caller
/// prepares or cleans - where a stage wants one (packaging does), it makes its own under the
/// system temp directory and removes it, so `make` can call this in parallel and rebuild
/// incrementally without two invocations colliding in a shared tree.
/// Refuse an option the chosen format would silently ignore.
///
/// An option that does nothing is how somebody believes they set one, so each is an error rather
/// than a no-op where it has no effect: `--category` and the title-metadata options only reach a
/// `title` or `pkg`, and `--privilege`/`--sdk` only reach a format that builds a container.
fn refuse_options_the_format_ignores(
    cli: &Cli,
    format: Format,
) -> Result<(), Box<dyn std::error::Error>> {
    let carries_metadata = matches!(format, Format::Title | Format::Pkg);
    let wraps = matches!(format, Format::Eboot | Format::Title | Format::Pkg);

    let metadata_only = [
        (cli.category.is_some(), "--category"),
        (cli.content_id.is_some(), "--content-id"),
        (cli.title_version.is_some(), "--title-version"),
        (cli.icon.is_some(), "--icon"),
        (cli.deeplink.is_some(), "--deeplink"),
    ];
    if !carries_metadata {
        for (set, name) in metadata_only {
            if set {
                return Err(format!(
                    "{name} applies to `title` and `pkg`, not to `{format:?}`; only a title \
                     carries the metadata it lands in"
                )
                .into());
            }
        }
    }

    let container_only = [
        (cli.privilege.is_some(), "--privilege", "declare a tier in"),
        (cli.sdk.is_some(), "--sdk", "pin a version in"),
    ];
    if !wraps {
        for (set, name, clause) in container_only {
            if set {
                return Err(format!(
                    "{name} applies to `eboot`, `title` and `pkg`, not to `{format:?}`; \
                     stamping produces no container to {clause}"
                )
                .into());
            }
        }
    }

    Ok(())
}

fn pipeline(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    // `requires_all` on `--input` means clap has already refused any partial set; these four
    // cannot be absent here, and an `expect` would be a panic in a binary that has removed
    // them everywhere else.
    let (Some(input), Some(target), Some(format), Some(output)) = (
        cli.input.as_deref(),
        cli.target,
        cli.format,
        cli.output.as_deref(),
    ) else {
        return Err("--input, --target, --format and --output are one invocation".into());
    };

    refuse_options_the_format_ignores(cli, format)?;
    let carries_metadata = matches!(format, Format::Title | Format::Pkg);

    let generation = target.generation();
    let category = cli.category.unwrap_or(Category::SystemApp);
    let privilege: selfish_container::Privilege = match cli.privilege.as_deref() {
        Some(tier) => tier.parse().map_err(|e: &str| e.to_owned())?,
        None => selfish_container::Privilege::App,
    };
    let sdk = cli.sdk.as_deref();
    let sdk_table = cli.sdk_table.as_deref();
    // Announced when it is the default, because a title id nobody chose is worth seeing in the
    // output rather than discovered in a directory name.
    let title_id = cli.title_id.as_deref().unwrap_or_else(|| {
        // Only where a title id is carried. An `elf`, `prx` or `eboot` has none, so announcing a
        // default there would report a choice nothing makes.
        if matches!(format, Format::Title | Format::Pkg) {
            say!("  id      {DEFAULT_TITLE_ID} (default; pass --title-id to choose)");
        }
        DEFAULT_TITLE_ID
    });
    let title = cli.title.as_deref().unwrap_or(title_id);

    // The content id defaults from the title id, announced like the title id itself, because a
    // `pkg` built with the wrong one produces a filesystem image a console cannot open - so a
    // silent default would be the quiet failure D073 warns about. Only built for the formats
    // that carry it.
    let default_content_id;
    let content_id: Option<&str> = match (carries_metadata, cli.content_id.as_deref()) {
        (_, Some(id)) => Some(id),
        (true, None) => {
            default_content_id = format!("UP0000-{title_id}_00-0000000000000000");
            say!("  content {default_content_id} (default; pass --content-id to choose)");
            Some(&default_content_id)
        }
        (false, None) => None,
    };

    say!("{target:?} -> {format:?}  ({generation})");

    let wrap = WrapOptions {
        privilege,
        sdk,
        sdk_table,
    };
    let meta = TitleMeta {
        content_id,
        version: cli.title_version.as_deref(),
        icon: cli.icon.as_deref(),
        deeplink: cli.deeplink.as_deref(),
    };
    match format {
        Format::Elf => stamped_elf(
            input,
            output,
            generation,
            selfish_elf::ObjectType::Executable,
        ),
        Format::Prx => stamped_elf(
            input,
            output,
            generation,
            selfish_elf::ObjectType::SharedLibrary,
        ),
        Format::Eboot => eboot(input, output, generation, &wrap),
        Format::Title => title_at(
            input, output, generation, title_id, title, category, &wrap, &meta,
        ),
        Format::Pkg => pkg_from_elf(
            input,
            output,
            generation,
            title_id,
            title,
            category,
            &cli.entries,
            &wrap,
            &meta,
        ),
    }
}

/// The title metadata a `title` or `pkg` carries beyond its name, for the formats that write a
/// `param.json` / `param.sfo`.
///
/// One struct for the same reason as [`WrapOptions`]: these travel together through `pkg` →
/// `title` → `title_dir`, and the content id travels on to `pack` as well.
struct TitleMeta<'a> {
    /// The content id, defaulted from the title id by the caller when a format carries it.
    content_id: Option<&'a str>,
    /// The title version, `NN.NN`; `None` keeps `title_dir`'s own default.
    version: Option<&'a str>,
    /// A PNG for the tile; `None` uses the generated default.
    icon: Option<&'a Path>,
    /// A launch URI; `None` builds a title that carries its own executable.
    deeplink: Option<&'a str>,
}

/// How a container is built, for the formats that build one.
///
/// Gathered into one struct rather than threaded as three arguments because they travel
/// together through every wrapping format and always have: `title` builds an `eboot` and `pkg`
/// builds a `title`, so anything the container needs has to survive two hops.
struct WrapOptions<'a> {
    /// The tier the container declares.
    privilege: selfish_container::Privilege,
    /// The SDK version or alias to pin, if one was asked for.
    sdk: Option<&'a str>,
    /// A replacement version table.
    sdk_table: Option<&'a Path>,
}

/// `--format elf`: the executable with the target's platform identity stamped into it.
fn stamped_elf(
    input: &Path,
    output: &Path,
    generation: Generation,
    kind: selfish_elf::ObjectType,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = std::fs::read(input)?;
    let elf = selfish_elf::Elf::parse(&bytes)?;
    say!(
        "  elf     {} program header(s), entry {:#x}",
        elf.program_headers().len(),
        elf.entry()
    );

    let changes = selfish_elf::identity::stamp(&mut bytes, kind, generation)?;
    for change in &changes {
        say!(
            "  stamp   {:<14} {:#x} -> {:#x}",
            change.field,
            change.from,
            change.to
        );
    }
    if changes.is_empty() {
        say!("  stamp   already {generation}");
    }

    write_exactly(output, &bytes)?;
    Ok(())
}

/// `--format eboot`: the executable, stamped, inside a container.
fn eboot(
    input: &Path,
    output: &Path,
    generation: Generation,
    wrap: &WrapOptions<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = std::fs::read(input)?;
    let changes =
        selfish_elf::identity::stamp(&mut bytes, selfish_elf::ObjectType::Executable, generation)?;
    say!("  stamp   {} field(s)", changes.len());

    // The default path stays `build`, which is the same call with an `App` tier and no version
    // patch. Routing everything through `build_with_options` would make every ordinary run pay
    // for a table load it does not read.
    let container = match (wrap.privilege, wrap.sdk) {
        (selfish_container::Privilege::App, None) => selfish_container::build(&bytes, generation)?,
        (privilege, sdk) => {
            let dict = selfish_container::SdkDictionary::load_or_embedded(wrap.sdk_table);
            // Defaulted rather than left absent, which is what `wrap` did: asking for a tier
            // and getting an unpinned version back would be a different artifact from the one
            // that verb produced.
            let target_sdk = match sdk {
                Some(text) => dict
                    .resolve(text, generation)
                    .map_err(Box::<dyn std::error::Error>::from)?,
                None => selfish_container::TargetSdk::default_for(generation),
            };
            say!("  tier    {privilege:?}");
            say!(
                "  sdk     0x{:08x}/0x{:08x}",
                target_sdk.orbis_sdk,
                target_sdk.ppr_sdk
            );
            selfish_container::build_with_options(&bytes, generation, privilege, Some(target_sdk))?
        }
    };
    say!(
        "  wrap    {} bytes from a {} byte payload",
        container.len(),
        bytes.len()
    );

    write_exactly(output, &container)?;
    Ok(())
}

/// `--format title`: a title directory, laid out as `<output>/<TITLE_ID>/`.
///
/// **The title id folder is part of the artifact, not decoration.** The install call is
/// `sceAppInstUtilAppInstallTitleDir("<TITLE_ID>", "<parent>", 0)` - a title id and the
/// directory *containing* it - so `<parent>/<TITLE_ID>/` is the shape the console asks for. An
/// earlier revision laid the title out at `--output` itself, on the reasoning that `--output`
/// is the artifact; that reasoning is tidy and it produced something the install call cannot
/// take.
///
/// So `--output` is the parent, and the artifact inside it is named by the title id. That is
/// still "nothing beside it": one directory is created and nothing else.
#[allow(
    clippy::too_many_arguments,
    reason = "a command-line command takes what the command line offers"
)]
fn title_at(
    input: &Path,
    output: &Path,
    generation: Generation,
    title_id: &str,
    title: &str,
    category: Category,
    wrap: &WrapOptions<'_>,
    meta: &TitleMeta<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Only the eboot needs staging - `title_dir` joins the title id on itself, so it can write
    // straight into `--output` and the directory shape comes out right with no move.
    let scratch = scratch_dir("title")?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let staged = scratch.join("root");
        std::fs::create_dir_all(&staged)?;
        eboot(input, &staged.join("eboot.bin"), generation, wrap)?;

        title_dir(
            output,
            title_id,
            title,
            meta.content_id,
            meta.version,
            meta.deeplink,
            meta.icon,
            Some(&staged),
            category.value(),
            wrap.privilege,
        )
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result?;
    say!("  title   {}", output.join(title_id).display());
    Ok(())
}

/// `--format pkg`: an installable package built from one executable.
///
/// **This is the stage that needs a directory**, because a package is made of a title layout
/// rather than of a file. The directory is made here, under the system temp, and removed - so
/// the option surface stays input-to-output and no caller prepares or cleans anything.
#[allow(
    clippy::too_many_arguments,
    reason = "a command-line command takes what the command line offers"
)]
fn pkg_from_elf(
    input: &Path,
    output: &Path,
    generation: Generation,
    title_id: &str,
    title: &str,
    category: Category,
    entries: &[String],
    wrap: &WrapOptions<'_>,
    meta: &TitleMeta<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch_dir("pkg")?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let laid = scratch.join("title");
        title_at(
            input, &laid, generation, title_id, title, category, wrap, meta,
        )?;
        // `title_at` writes `<laid>/<TITLE_ID>/`, and a package is made of the title's own
        // contents rather than of the directory holding it. The content id has to match the one
        // the title was laid out with - the image is keyed by it - so both come from `meta`,
        // which `run` has already defaulted for a format that carries metadata.
        let contents = laid.join(title_id);

        // `--icon` set the mounted `sce_sys/icon0.png` through `title_at`; for a package it also
        // has to fill the `0x1200` store tile, or the caller's art shows after install while the
        // default mark shows in the store. Injected as a `0x1200` entry so it goes through
        // `pack`'s own `normalise`. Refused alongside an explicit `--entry 0x1200=`, which would
        // be two sources for one tile.
        let mut entries: Vec<String> = entries.to_vec();
        if let Some(icon) = meta.icon {
            if builder_ids(&entries).contains(&ICON_ENTRY) {
                return Err(
                    "--icon and --entry 0x1200= both set the store tile; pass one or the other"
                        .into(),
                );
            }
            entries.push(format!("{ICON_ENTRY:#x}={}", icon.display()));
        }

        pack(
            None,
            Some(&contents),
            None,
            output,
            meta.content_id.unwrap_or(""),
            &entries,
            Some(title_id),
            Some(title),
            meta.version.unwrap_or("01.00"),
        )
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result?;
    Ok(())
}

/// Write a file at exactly this path, creating only the directories above it.
fn write_exactly(output: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, bytes)?;
    say!("  wrote   {} ({} bytes)", output.display(), bytes.len());
    Ok(())
}

/// A private scratch directory under the system temp, named so two runs cannot collide.
///
/// Under the system temp rather than beside `--output`, so a parallel `make` never sees a
/// half-built directory in its own tree, and so a failed run leaves nothing in the caller's.
fn scratch_dir(what: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("selfish-{what}-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Lay out a Prospero title directory.
#[allow(
    clippy::too_many_arguments,
    reason = "a command-line command takes what the command line offers"
)]
fn title_dir(
    out: &Path,
    title_id: &str,
    title: &str,
    content_id: Option<&str>,
    version: Option<&str>,
    deeplink: Option<&str>,
    icon: Option<&Path>,
    root: Option<&Path>,
    category: i64,
    privilege: selfish_container::Privilege,
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
    let category = match privilege {
        selfish_container::Privilege::System => 0x20000,
        _ => category,
    };
    say!("privilege: {privilege:?}");
    param.set_prospero(
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
        // Normalised, not copied. A console wants 512x512 with no alpha, and an icon that is
        // neither is accepted and then composited wrongly - square on a home screen, found on a
        // television. So a supplied icon goes through the same conversion the `pack` entry path
        // uses, rather than being written as authored. (D073)
        let raw = std::fs::read(path)?;
        let converted = icon::normalise(&raw, &path.display().to_string())?;
        std::fs::write(&icon_path, &converted)?;
        if converted.len() == raw.len() {
            say!("{} (from {})", icon_path.display(), path.display());
        } else {
            say!(
                "{} (from {}, converted to 512x512 RGB)",
                icon_path.display(),
                path.display()
            );
        }
    } else {
        std::fs::write(&icon_path, icon::default_icon()?)?;
        say!("{} (generated)", icon_path.display());
    }

    let keystone_path = sce_sys.join("keystone");
    if !keystone_path.exists() {
        let keystone = selfish_pkg::keystone::create(selfish_pkg::keys::FAKE_PASSCODE)?;
        std::fs::write(&keystone_path, keystone)?;
        say!("{} (generated)", keystone_path.display());
    }

    let pfs_ver_path = sce_sys.join("pfs-version.dat");
    if !pfs_ver_path.exists() {
        std::fs::write(&pfs_ver_path, b"01.004.000")?;
        say!("{} (generated)", pfs_ver_path.display());
    }

    let nptitle_path = sce_sys.join("nptitle.dat");
    if !nptitle_path.exists() {
        let mut nptd = vec![0_u8; 160];
        if let Some(prefix) = nptd.get_mut(..4) {
            prefix.copy_from_slice(b"NPTD");
        }
        if let Some(flag) = nptd.get_mut(7) {
            *flag = 0x80;
        }
        let np_id = format!("{title_id}_00");
        let len = np_id.len().min(16);
        let end = 16usize.saturating_add(len);
        if let (Some(dst), Some(src)) = (nptd.get_mut(16..end), np_id.as_bytes().get(..len)) {
            dst.copy_from_slice(src);
        }
        std::fs::write(&nptitle_path, nptd)?;
        say!("{} (generated)", nptitle_path.display());
    }

    // No install advice here. This lays a directory out and reports where things went; the
    // pipeline calls it into a scratch directory it then moves, so advice naming `base` would
    // name a path that no longer exists by the time anybody read it. `selfish title` reports it instead.
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

/// `selfish shader`: an AGC compute shader container from a caller's bytecode size and registers.
///
/// The build-time counterpart of `selfish-shader` for a consumer that is not Rust: obscene's
/// probe generates its container here rather than carrying a copy of the layout. (D103)
#[allow(
    clippy::too_many_arguments,
    reason = "a command-line command takes what the command line offers"
)]
fn shader(
    stage: &str,
    code: Option<&Path>,
    shader_size: Option<u32>,
    target: &str,
    sh_registers: &[String],
    out: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // A named stage has a value a citable source establishes; anything else is passed through as a
    // raw type value the caller has confirmed itself, which is exactly what `for_stage` is for.
    let (stage_value, stage_label) = match stage {
        "compute" => (selfish_shader::STAGE_COMPUTE, "compute".to_owned()),
        "pixel" => (selfish_shader::STAGE_PIXEL, "pixel".to_owned()),
        "vertex" => (selfish_shader::STAGE_VERTEX, "vertex".to_owned()),
        other => {
            let value = hex_or_dec_u32(other)
                .and_then(|v| u8::try_from(v).ok())
                .ok_or_else(|| format!("bad --stage {other:?}: a name or a 0-255 type value"))?;
            (value, format!("type {value:#x}"))
        }
    };

    let size = match (code, shader_size) {
        (Some(path), _) => {
            let bytes = std::fs::read(path)?;
            u32::try_from(bytes.len())
                .map_err(|_| "the shader bytecode is larger than shader_size (a u32) can record")?
        }
        (None, Some(size)) => size,
        (None, None) => return Err("one of --code or --shader-size is required".into()),
    };

    let target = hex_or_dec_u32(target).ok_or_else(|| format!("bad --target {target:?}"))?;

    let mut registers = Vec::with_capacity(sh_registers.len());
    for spec in sh_registers {
        let (offset, value) = spec
            .split_once('=')
            .ok_or_else(|| format!("--sh-reg wants OFFSET=VALUE, got {spec:?}"))?;
        registers.push(selfish_shader::ShaderRegister {
            offset: hex_or_dec_u32(offset)
                .ok_or_else(|| format!("bad register offset {offset:?}"))?,
            value: hex_or_dec_u32(value).ok_or_else(|| format!("bad register value {value:?}"))?,
        });
    }

    // A console rejects a shader whose SH register table lacks its stage's program-address pair
    // (9f4c: num_sh_registers = 0 is INVALID_STAGE_REGISTERS). Warned rather than refused, because
    // this crate lays out what it is handed and does not own that rule. The pair's offsets are
    // stage-specific - compute's are 0x20c/0x20d - so only compute gets the concrete hint. (D103)
    if registers.len() < 2 {
        let hint = if stage_value == selfish_shader::STAGE_COMPUTE {
            " - for compute that is 0x20c=0, 0x20d=0"
        } else {
            ""
        };
        say!(
            "warning: {} SH register(s). A console needs at least the stage's program-address pair, \
             which it patches from the code pointer, plus the shader's own resource registers{hint} \
             - pass them with --sh-reg",
            registers.len()
        );
    }

    let container =
        selfish_shader::Container::for_stage(stage_value, size, target, registers).build();
    write_exactly(out, &container)?;
    say!(
        "{}: {} byte AGC {stage_label} container (shader_size {size}, target {target:#x})",
        out.display(),
        container.len()
    );
    Ok(())
}

/// A `0x`-prefixed hexadecimal or plain decimal `u32`.
fn hex_or_dec_u32(text: &str) -> Option<u32> {
    let trimmed = text.trim();
    trimmed.strip_prefix("0x").map_or_else(
        || trimmed.parse::<u32>().ok(),
        |hex| u32::from_str_radix(hex, 16).ok(),
    )
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

/// Build the image a package carries, from a tree.
///
/// Three layers, and the keys for the outer one come from the content id and the passcode.
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
    if let Some(inner) = selfish_pkg::write::inner_image_size(&image, content_id, &passcode)
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
