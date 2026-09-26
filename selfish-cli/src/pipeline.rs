//! A pipeline invocation: `--input` through `--format` to `--output`.
//!
//! Every format is a pure function of its inputs: it reads `--input`, writes exactly `--output`
//! and puts nothing beside it. A stage that needs a staging directory makes its own under the
//! system temp directory and removes it, so parallel invocations never collide.

use std::path::{Path, PathBuf};

use selfish_abi::Generation;

use crate::Result;
use crate::args::{Category, Cli, Format};
use crate::layout::{self, Title};
use crate::pack::{self, ICON_ENTRY};

/// The title id when `--title-id` is not given.
const DEFAULT_TITLE_ID: &str = "OBSC00001";

/// The title version a package carries when none is given.
const DEFAULT_VERSION: &str = "01.00";

/// The title metadata a `title` or `pkg` carries beyond its name.
pub(crate) struct TitleMeta<'a> {
    /// The content id, defaulted from the title id for a format that carries it.
    pub(crate) content_id: Option<&'a str>,
    /// The title version, `NN.NN`.
    pub(crate) version: Option<&'a str>,
    /// A PNG for the tile; `None` uses the generated default.
    pub(crate) icon: Option<&'a Path>,
    /// A launch URI; `None` builds a title that carries its own executable.
    pub(crate) deeplink: Option<&'a str>,
    /// A PNG for the background (`pic0.png`); `None` uses the default.
    pub(crate) pic0: Option<&'a Path>,
    /// A transparent PNG for the title logo (`logo.png`); `None` uses the default.
    pub(crate) logo: Option<&'a Path>,
    /// The subtitle; `None` uses the default.
    pub(crate) subtitle: Option<&'a str>,
}

/// How a container is built, for the formats that build one. `title` builds an `eboot` and
/// `pkg` builds a `title`, so these travel through both.
struct WrapOptions<'a> {
    /// The tier the container declares.
    privilege: selfish_container::Privilege,
    /// The SDK version or alias to pin.
    sdk: Option<&'a str>,
    /// A replacement version table.
    sdk_table: Option<&'a Path>,
}

/// Everything one build needs, resolved from the command line.
struct Build<'a> {
    input: &'a Path,
    generation: Generation,
    title_id: &'a str,
    title: &'a str,
    category: Category,
    entries: &'a [String],
    wrap: WrapOptions<'a>,
    meta: TitleMeta<'a>,
}

/// Resolve the options and run the chosen format.
pub(crate) fn run(cli: &Cli) -> Result {
    // clap requires all four together.
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

    let privilege = match cli.privilege.as_deref() {
        Some(tier) => tier.parse().map_err(|e: &str| e.to_owned())?,
        None => selfish_container::Privilege::App,
    };
    let title_id = cli.title_id.as_deref().unwrap_or_else(|| {
        if carries_metadata {
            say!("  id      {DEFAULT_TITLE_ID} (default; pass --title-id to choose)");
        }
        DEFAULT_TITLE_ID
    });
    if carries_metadata {
        check_title_id(title_id)?;
    }

    // Announced when defaulted: a `pkg` built with the wrong content id carries an image the
    // hardware cannot open.
    let default_content_id;
    let content_id = match (carries_metadata, cli.content_id.as_deref()) {
        (_, Some(id)) => Some(id),
        (true, None) => {
            default_content_id = format!("UP0000-{title_id}_00-0000000000000000");
            say!("  content {default_content_id} (default; pass --content-id to choose)");
            Some(default_content_id.as_str())
        }
        (false, None) => None,
    };

    let generation = target.generation();
    say!("{target:?} -> {format:?}  ({generation})");
    let build = Build {
        input,
        generation,
        title_id,
        title: cli.title.as_deref().unwrap_or(title_id),
        category: cli.category.unwrap_or(Category::SystemApp),
        entries: &cli.entries,
        wrap: WrapOptions {
            privilege,
            sdk: cli.sdk.as_deref(),
            sdk_table: cli.sdk_table.as_deref(),
        },
        meta: TitleMeta {
            content_id,
            version: cli.title_version.as_deref(),
            icon: cli.icon.as_deref(),
            deeplink: cli.deeplink.as_deref(),
            pic0: cli.pic0.as_deref(),
            logo: cli.logo.as_deref(),
            subtitle: cli.subtitle.as_deref(),
        },
    };
    match format {
        Format::Elf => stamped_elf(&build, output, selfish_elf::ObjectType::Executable),
        Format::Prx => stamped_elf(&build, output, selfish_elf::ObjectType::SharedLibrary),
        Format::Eboot => eboot(&build, output),
        Format::Title => title(&build, output),
        Format::Pkg => pkg(&build, output),
    }
}

/// Refuse an option the chosen format would silently ignore.
///
/// `--category` and the title metadata options reach only a `title` or `pkg`, and `--privilege`
/// and `--sdk` only a format that builds a container.
fn refuse_options_the_format_ignores(cli: &Cli, format: Format) -> Result {
    let carries_metadata = matches!(format, Format::Title | Format::Pkg);
    let wraps = matches!(format, Format::Eboot | Format::Title | Format::Pkg);

    let metadata_only = [
        (cli.category.is_some(), "--category"),
        (cli.content_id.is_some(), "--content-id"),
        (cli.title_version.is_some(), "--title-version"),
        (cli.icon.is_some(), "--icon"),
        (cli.deeplink.is_some(), "--deeplink"),
    ];
    if let Some((_, name)) = metadata_only.iter().find(|(set, _)| *set)
        && !carries_metadata
    {
        return Err(format!(
            "{name} applies to `title` and `pkg`, not to `{format:?}`; only a title \
             carries the metadata it lands in"
        )
        .into());
    }

    let container_only = [
        (cli.privilege.is_some(), "--privilege", "declare a tier in"),
        (cli.sdk.is_some(), "--sdk", "pin a version in"),
    ];
    if let Some((_, name, clause)) = container_only.iter().find(|(set, _, _)| *set)
        && !wraps
    {
        return Err(format!(
            "{name} applies to `eboot`, `title` and `pkg`, not to `{format:?}`; \
             stamping produces no container to {clause}"
        )
        .into());
    }
    Ok(())
}

/// A title id is four capital letters then five digits.
///
/// It names the directory the hardware indexes titles by, and a malformed one installs cleanly
/// and is then never indexed: the hardware logs `Invalid TitleId` and nothing else says so.
fn check_title_id(title_id: &str) -> Result {
    if title_id.len() == 9
        && let Some((letters, digits)) = title_id.split_at_checked(4)
        && letters.bytes().all(|b| b.is_ascii_uppercase())
        && digits.bytes().all(|b| b.is_ascii_digit())
    {
        return Ok(());
    }
    Err(format!(
        "--title-id {title_id} is not a title id: it must be four capital letters then five \
         digits, like GLCB00001. A malformed id installs cleanly and is then never indexed"
    )
    .into())
}

/// `--format elf` and `--format prx`: the executable with the target's identity stamped in.
fn stamped_elf(build: &Build<'_>, output: &Path, kind: selfish_elf::ObjectType) -> Result {
    let mut bytes = std::fs::read(build.input)?;
    let elf = selfish_elf::Elf::parse(&bytes)?;
    say!(
        "  elf     {} program header(s), entry {:#x}",
        elf.program_headers().len(),
        elf.entry()
    );

    let generation = build.generation;
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
    write_exactly(output, &bytes)
}

/// `--format eboot`: the executable, stamped, inside a container.
///
/// An `App` tier with no version pin takes `build`, which loads no version table.
fn eboot(build: &Build<'_>, output: &Path) -> Result {
    let generation = build.generation;
    let mut bytes = std::fs::read(build.input)?;
    let changes =
        selfish_elf::identity::stamp(&mut bytes, selfish_elf::ObjectType::Executable, generation)?;
    say!("  stamp   {} field(s)", changes.len());

    let wrap = &build.wrap;
    let container = match (wrap.privilege, wrap.sdk) {
        (selfish_container::Privilege::App, None) => selfish_container::build(&bytes, generation)?,
        (privilege, sdk) => {
            let dict = selfish_container::SdkDictionary::load_or_embedded(wrap.sdk_table);
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
    write_exactly(output, &container)
}

/// `--format title`: a title directory at `<output>/<TITLE_ID>/`.
///
/// The install call takes a title id and the directory containing it, so `--output` is the
/// parent and the one directory created in it is named by the title id.
fn title(build: &Build<'_>, output: &Path) -> Result {
    let scratch = scratch_dir("title")?;
    let result = (|| -> Result {
        let staged = scratch.join("root");
        std::fs::create_dir_all(&staged)?;
        eboot(build, &staged.join("eboot.bin"))?;
        let title = Title {
            id: build.title_id,
            name: build.title,
            category: build.category.value(),
            privilege: build.wrap.privilege,
        };
        layout::title_dir(output, &title, Some(&staged), &build.meta)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result?;
    say!("  title   {}", output.join(build.title_id).display());
    Ok(())
}

/// `--format pkg`: an installable package built from one executable, by way of a title laid
/// out in a scratch directory.
fn pkg(build: &Build<'_>, output: &Path) -> Result {
    let scratch = scratch_dir("pkg")?;
    let result = (|| -> Result {
        let laid = scratch.join("title");
        title(build, &laid)?;
        let contents = laid.join(build.title_id);

        // `--icon` also fills the `0x1200` store tile, through `pack`'s own conversion.
        let mut entries = build.entries.to_vec();
        if let Some(icon) = build.meta.icon {
            if pack::entry_ids(&entries).contains(&ICON_ENTRY) {
                return Err(
                    "--icon and --entry 0x1200= both set the store tile; pass one or the other"
                        .into(),
                );
            }
            entries.push(format!("{ICON_ENTRY:#x}={}", icon.display()));
        }

        pack::pack(&pack::Request {
            image: None,
            dir: Some(&contents),
            passcode: None,
            out: output,
            content_id: build.meta.content_id.unwrap_or(""),
            entries: &entries,
            title_id: Some(build.title_id),
            title: Some(build.title),
            version: build.meta.version.unwrap_or(DEFAULT_VERSION),
        })
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

/// Write a file at exactly this path, creating only the directories above it.
pub(crate) fn write_exactly(output: &Path, bytes: &[u8]) -> Result {
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
fn scratch_dir(what: &str) -> Result<PathBuf> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("selfish-{what}-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
