//! The command line: a pipeline invocation (`--input` through `--output`) or a diagnostic
//! subcommand.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use selfish_abi::Generation;

#[derive(Parser)]
#[command(
    name = "selfish",
    version = oops_build::line!(),
    about = "Read and write the file formats Prospero-generation hardware loads"
)]
pub(crate) struct Cli {
    /// The executable to build from. Begins a pipeline invocation.
    ///
    /// `--input`, `--target`, `--format` and `--output` are one pipeline run and are required
    /// together. With none of them, `selfish` takes a diagnostic subcommand instead.
    #[arg(long, value_name = "FILE", requires_all = ["target", "format", "output"])]
    pub(crate) input: Option<PathBuf>,

    /// Which machine the artifact is for.
    #[arg(long, value_enum, requires = "input")]
    pub(crate) target: Option<Target>,

    /// What to produce.
    #[arg(long, value_enum, requires = "input")]
    pub(crate) format: Option<Format>,

    /// Where to write it. Written exactly, with nothing placed beside it.
    #[arg(long, value_name = "PATH", requires = "input")]
    pub(crate) output: Option<PathBuf>,

    /// What the title declares itself to be. `--format title` and `--format pkg` only.
    ///
    /// Defaults to `system-app`.
    #[arg(long, value_enum, requires = "input")]
    pub(crate) category: Option<Category>,

    /// Privilege tier the container declares: app, sysmodule, system, or root.
    ///
    /// Refused for `--format elf` and `--format prx`, which produce no container. `sysmodule`
    /// writes the same container `app` does. `system` also sets the title's category to
    /// `0x20000`.
    #[arg(long, value_name = "TIER", requires = "input")]
    pub(crate) privilege: Option<String>,

    /// Target SDK version or alias to pin, such as `2.000.009`, `prospero` or `orbis`.
    ///
    /// Patches `PT_SCE_PROCPARAM` and the container's declared version. Same formats as
    /// `--privilege`, and the same refusal for `elf` and `prx`.
    #[arg(long, value_name = "VERSION", requires = "input")]
    pub(crate) sdk: Option<String>,

    /// A `sdk-versions.toml` to read instead of the embedded one.
    #[arg(long, value_name = "FILE", requires = "sdk")]
    pub(crate) sdk_table: Option<PathBuf>,

    /// The title id an artifact carries: four capital letters then five digits. Defaults to
    /// `OBSC00001`, and says so.
    ///
    /// An ELF has no field for a title id: `PT_SCE_PROCPARAM` and `PT_SCE_MODULE_PARAM` hold SDK
    /// versions and no identity. The id is title metadata, carried by `param.json` (`titleId`),
    /// `PARAM.SFO` (`TITLE_ID`) and a package's content id, so it is an option here.
    #[arg(long, requires = "input")]
    pub(crate) title_id: Option<String>,

    /// The name shown on the home screen. Defaults to the title id.
    #[arg(long, requires = "input")]
    pub(crate) title: Option<String>,

    /// The content id, such as `UP0000-PPSA01650_00-YOUTUBE000000000`. `--format title` and
    /// `--format pkg` only.
    ///
    /// For `pkg` the filesystem image is keyed by a hash of the content id, so a package built
    /// with the wrong one carries an image the hardware cannot open. It also lands in
    /// `param.json` and `param.sfo`. Defaults to an id derived from the title id.
    #[arg(long, value_name = "ID", requires = "input")]
    pub(crate) content_id: Option<String>,

    /// The title version, as `NN.NN`. `--format title` and `--format pkg` only.
    ///
    /// Sets the version and master version in `param.json` and `param.sfo`. Defaults to
    /// `01.00`. Spelled `--title-version` because `--version` is the tool's own.
    #[arg(long = "title-version", value_name = "NN.NN", requires = "input")]
    pub(crate) title_version: Option<String>,

    /// A 512x512 PNG for the home-screen tile. `--format title` and `--format pkg` only.
    ///
    /// Flattened to RGB with no alpha, which the hardware requires. A PNG of any other size is
    /// refused rather than scaled. For `pkg` it fills both the mounted tile and the `0x1200`
    /// store tile. Without one, selfish's own mark is used. (D073)
    #[arg(long, value_name = "FILE", requires = "input")]
    pub(crate) icon: Option<PathBuf>,

    /// A 1920x1080 or 3840x2160 PNG for the home-screen background. `--format title` only.
    ///
    /// Flattened to RGB over black. Without one, a default background is used.
    #[arg(long, value_name = "FILE", requires = "input")]
    pub(crate) pic0: Option<PathBuf>,

    /// A transparent RGBA PNG for the title logo. `--format title` only.
    ///
    /// Drawn on the lower left above the action buttons. Without one, a default badge is used.
    #[arg(long, value_name = "FILE", requires = "input")]
    pub(crate) logo: Option<PathBuf>,

    /// Subtitle text for the title. `--format title` only.
    ///
    /// Stored in `param.json`'s `titleSubName`. Defaults to "OOPS Native Title".
    #[arg(long, value_name = "TEXT", requires = "input")]
    pub(crate) subtitle: Option<String>,

    /// Make the entry launch a URI instead of carrying its own executable. `--format title` and
    /// `--format pkg` only.
    ///
    /// A launcher tile, for code already running as a payload. Sets `param.json`'s deeplink.
    #[arg(long, value_name = "URI", requires = "input")]
    pub(crate) deeplink: Option<String>,

    /// A package entry, as `ID=FILE`. `--format pkg` only.
    ///
    /// Overrides an entry this tool would otherwise compute or generate, for rebuilding a
    /// package to match existing material. No entry is required. (D099)
    #[arg(long = "entry", value_name = "ID=FILE", requires = "input")]
    pub(crate) entries: Vec<String>,

    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

/// Which machine an artifact is for. The generation follows from it.
///
/// `neo` and `trinity` are the mid-generation refreshes and are not synonyms for `orbis` and
/// `prospero`. An artifact for any previous-generation machine is `orbis`; `neo` is only for a
/// Pro-specific build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "lower")]
pub(crate) enum Target {
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
    pub(crate) const fn generation(self) -> Generation {
        match self {
            Self::Orbis | Self::Neo => Generation::Orbis,
            Self::Prospero | Self::Trinity => Generation::Prospero,
        }
    }
}

/// What to produce.
///
/// Each is a pure function of `--input` and the other options: it writes exactly `--output`
/// and nothing beside it, and needs no directory the caller prepares or cleans.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "lower")]
pub(crate) enum Format {
    /// The executable, stamped with the target's platform identity. No container.
    Elf,
    /// A shared library, stamped as one. No container. The same stamping as `elf` with a
    /// different `e_type`.
    Prx,
    /// A signed-executable container: an `eboot.bin`.
    Eboot,
    /// A title directory, laid out as `<output>/<TITLE_ID>/`, the shape the install call takes.
    Title,
    /// An installable package.
    Pkg,
}

/// What a title declares itself to be.
///
/// The loader decides an artifact's memory budget and whether it owns the display from this,
/// and refuses a previous-generation category the current generation's libraries. Only
/// `--format title` and `--format pkg` carry it; any other format refuses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "kebab-case")]
pub(crate) enum Category {
    /// Full memory budget and exclusive primary display. A game.
    BigApp,
    /// No direct memory and no primary display. The default: it needs neither `libc.prx` nor a
    /// `pltauth` patch.
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
    pub(crate) const fn value(self) -> i64 {
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
pub(crate) enum Command {
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
    /// `data/self-format.tsv` is derived from previous-generation sources, so every row is a
    /// hypothesis until a current-generation file confirms or refutes it. This reports which
    /// fixed header rows a real `eboot.bin` agrees with and which it contradicts. A
    /// contradiction is a finding, not a fact: settling a field needs a citable source. (D084)
    ///
    /// The input may be a whole container or just its header region; a few kilobytes is enough.
    Audit {
        /// The container, or a dump of its header region.
        file: PathBuf,
    },
    /// Build a filesystem image from a directory of files.
    ///
    /// The files become a plain filesystem, wrapped in a `PFSC` container, carried as the single
    /// file of a signed and encrypted outer filesystem.
    ///
    /// The image is encrypted under a key derived from the content id and the passcode, so it
    /// opens only in a package that declares the same content id.
    Image {
        /// The directory to build from: the root of what the title mounts.
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
    /// Assemble a package.
    ///
    /// Everything derivable is computed. An entry nothing here can compute must be handed in
    /// with `--entry`, and the build refuses rather than inventing it.
    Pack {
        /// The filesystem image, already built. Use `--dir` instead to build one here.
        #[arg(long, conflicts_with = "dir", required_unless_present = "dir")]
        image: Option<PathBuf>,
        /// A directory of files to build the image from: the root of what the title mounts.
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
        /// An entry this tool cannot compute, as `ID=FILE`, for example `0x400=blob.bin`.
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
    /// Some entries are established by derivation rather than taken from a source. This re-runs
    /// that derivation against any packages you have, so the format table needs no trust.
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
    /// Build an AGC shader container: the header `sceAgcCreateShader` is handed.
    ///
    /// The container format is this tool's (`data/agc-shader-format.tsv`, D103); the register
    /// contents are the shader's, read from its bytecode by whoever produced it and passed in
    /// with `--sh-reg`. The bytecode is not embedded, since the hardware fills the code pointer
    /// at create time; its size is recorded.
    ///
    /// `compute`, `pixel` and `vertex` have citable type values; any other stage takes a raw
    /// type value the caller has confirmed itself.
    Shader {
        /// The shader stage: `compute` (default), `pixel`, `vertex`, or a raw `type` value (hex
        /// or decimal).
        #[arg(long, default_value = "compute")]
        stage: String,
        /// The compiled shader bytecode. Its length is recorded as `shader_size`; the bytes are
        /// not embedded. Use `--shader-size` instead if you only have the size.
        #[arg(long, value_name = "FILE")]
        code: Option<PathBuf>,
        /// The bytecode size, when `--code` is not given. One of the two is required.
        #[arg(long, value_name = "BYTES")]
        shader_size: Option<u32>,
        /// The ISA target the bytecode is for. Defaults to `0x0e` (RDNA2).
        #[arg(long, value_name = "N", default_value = "0x0e")]
        target: String,
        /// An SH register, as `OFFSET=VALUE` (hex or decimal), repeatable. A compute shader
        /// needs at least the program-address pair `0x20c=0` and `0x20d=0`, which the hardware
        /// patches from the code pointer; its resource registers come from the bytecode.
        #[arg(long = "sh-reg", value_name = "OFFSET=VALUE")]
        sh_registers: Vec<String>,
        /// Where to write the container.
        #[arg(long, short)]
        out: PathBuf,
    },
}
