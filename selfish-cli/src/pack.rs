//! `image` and `pack`: building a filesystem image and a package.

use std::path::Path;

use selfish_pfs::write::Tree;
use selfish_pkg::write::Builder;

use crate::Result;
use crate::shader::hex_or_dec_u32;

/// The icon entry. It has no name in `entry_id` because no evidence identifies it.
pub(crate) const ICON_ENTRY: u32 = 0x1200;
/// The playgo manifest entry.
const PLAYGO_MANIFEST_ENTRY: u32 = 0x1003;
/// The playgo manifest a single-chunk title carries: one chunk, one scenario, opened with a
/// UTF-8 BOM. `scePlayGoCoreGetRawContentInfo` reads the chunk and scenario layout from it and
/// fails with `0x80f00200` on a manifest that has neither. Checked against a real package.
const PLAYGO_MANIFEST: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\n<psproject fmt=\"playgo-manifest\" version=\"0990\">\n  <volume>\n    <chunk_info chunk_count=\"1\" scenario_count=\"1\">\n      <scenarios default_id=\"0\">\n        <scenario id=\"0\" type=\"sp\" initial_chunk_count=\"1\" label=\"Scenario #0\">0</scenario>\n      </scenarios>\n    </chunk_info>\n  </volume>\n</psproject>\n";
/// The directory a title's own metadata lives in.
const SCE_SYS: &str = "sce_sys";
/// The block size every image here is built with.
const BLOCK: u32 = 0x10000;

/// The passcode as bytes: the one given, or the fake one.
fn passcode_bytes(passcode: Option<&str>) -> Vec<u8> {
    passcode.map_or_else(
        || selfish_pkg::keys::FAKE_PASSCODE.to_vec(),
        |text| text.as_bytes().to_vec(),
    )
}

/// `selfish image`: build a filesystem image from a directory, and say what it is keyed to.
pub(crate) fn image(root: &Path, out: &Path, content_id: &str, passcode: Option<&str>) -> Result {
    let passcode = passcode_bytes(passcode);
    let image = build_image(&read_tree(root)?, content_id, &passcode)?;
    std::fs::write(out, &image)?;
    say!("{}: {} bytes", out.display(), image.len());
    say!("keyed to {content_id} - a package carrying this must declare the same id");
    Ok(())
}

/// Read a directory into a tree the filesystem writer understands.
///
/// Sorted, so a package built twice from the same directory is the same package.
fn read_tree(root: &Path) -> Result<Tree> {
    fn walk(at: &Path, name: &str) -> Result<Tree> {
        let mut tree = Tree::new(name);
        let mut entries: Vec<_> = std::fs::read_dir(at)?.collect::<std::io::Result<Vec<_>>>()?;
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

/// Build the image a package carries: a plain filesystem, in a `PFSC` container, as the one
/// file of an outer filesystem encrypted under keys from the content id and the passcode.
///
/// Every package carries `sce_sys/keystone`, derived from the passcode, so one is added unless
/// the tree already has it.
fn build_image(tree: &Tree, content_id: &str, passcode: &[u8]) -> Result<Vec<u8>> {
    let mut tree = tree.clone();
    let has_keystone = tree
        .dirs
        .iter()
        .filter(|dir| dir.name == SCE_SYS)
        .any(|dir| dir.files.iter().any(|(name, _)| name == "keystone"));
    if has_keystone {
        say!("sce_sys/keystone: supplied by the caller, left alone");
    } else {
        let keystone = selfish_pkg::keystone::create(passcode)?;
        match tree.dirs.iter_mut().find(|dir| dir.name == SCE_SYS) {
            Some(dir) => dir.files.push(("keystone".to_owned(), keystone)),
            None => tree
                .dirs
                .push(Tree::new(SCE_SYS).with_file("keystone", keystone)),
        }
        say!("sce_sys/keystone: generated from the passcode");
    }

    let inner = selfish_pfs::write::build(&tree, BLOCK)?;
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

/// The `pack` subcommand's arguments.
pub(crate) struct Request<'a> {
    pub(crate) image: Option<&'a Path>,
    pub(crate) dir: Option<&'a Path>,
    pub(crate) passcode: Option<&'a str>,
    pub(crate) out: &'a Path,
    pub(crate) content_id: &'a str,
    pub(crate) entries: &'a [String],
    pub(crate) title_id: Option<&'a str>,
    pub(crate) title: Option<&'a str>,
    pub(crate) version: &'a str,
}

/// `selfish pack`: the image, the caller's entries, a default for each required entry the
/// caller left out, then the package and a report of any bytes nothing accounts for.
pub(crate) fn pack(request: &Request<'_>) -> Result {
    let passcode = passcode_bytes(request.passcode);
    let image = match (request.image, request.dir) {
        (Some(path), _) => std::fs::read(path)?,
        (None, Some(root)) => {
            let image = build_image(&read_tree(root)?, request.content_id, &passcode)?;
            say!("image: {} bytes from {}", image.len(), root.display());
            image
        }
        (None, None) => return Err("one of --image or --dir is required".into()),
    };
    warn_if_inner_too_small(&image, request.content_id, &passcode);

    let builder = Builder::new()
        .content_id(request.content_id)
        .passcode(&passcode)
        .image(image);
    let builder = supplied_entries(builder, request.entries)?;
    let builder = default_entries(builder, request)?;

    // Printed as-is and without the `selfish:` prefix: the error is a list of missing entries.
    let built = match builder.build() {
        Ok(built) => built,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    std::fs::write(request.out, &built.bytes)?;
    say!(
        "{}: {} bytes, {} entries, image at {:#x}",
        request.out.display(),
        built.bytes.len(),
        built.entries,
        built.image_at
    );
    report_gaps(&built);
    Ok(())
}

/// Warn when the inner filesystem is too small to mount.
///
/// The hardware refuses a small inner filesystem after the outer image has mounted, with
/// `Failed to enable GDDR5 cache` and `EINVAL`. Lowering the header's declared cache size does
/// not help; the inner filesystem itself has to be larger. (D071)
fn warn_if_inner_too_small(image: &[u8], content_id: &str, passcode: &[u8]) {
    let limit = selfish_pkg::write::DEFAULT_CACHE_SIZE;
    if let Some(inner) = selfish_pkg::write::inner_image_size(image, content_id, passcode)
        .filter(|inner| *inner < u64::from(limit))
    {
        say!(
            "warning: the inner filesystem is {inner} bytes. The hardware refuses to mount an image \
             this small - `Failed to enable GDDR5 cache`, EINVAL, after the outer image has \
             already mounted - and lowering the declared cache size does NOT help: it was tried, \
             set to exactly {inner}, and the hardware refused it identically. Pad the directory \
             past {limit} bytes."
        );
    }
}

/// Add each `ID=FILE` entry. An icon is converted to the 512x512 RGB the hardware draws
/// correctly. (D073)
fn supplied_entries(mut builder: Builder, specs: &[String]) -> Result<Builder> {
    for spec in specs {
        let (id, path) = spec
            .split_once('=')
            .ok_or_else(|| format!("--entry wants ID=FILE, got {spec:?}"))?;
        let id = hex_or_dec_u32(id).ok_or_else(|| format!("bad entry id {id:?}"))?;
        let mut bytes = std::fs::read(path)?;
        if id == ICON_ENTRY {
            let converted = crate::icon::normalise(&bytes, path)?;
            if converted.len() == bytes.len() {
                say!("icon0.png: {path}");
            } else {
                say!("icon0.png: {path}, converted to 512x512 RGB for the hardware");
            }
            bytes = converted;
        }
        builder = builder.entry(id, bytes);
    }
    Ok(builder)
}

/// Fill each required entry the caller did not supply, and say so.
///
/// A `param.sfo` is a format, so a real one is generated. The icon is selfish's own mark. The
/// playgo manifest is the single-chunk default.
fn default_entries(mut builder: Builder, request: &Request<'_>) -> Result<Builder> {
    let supplied = entry_ids(request.entries);
    if !supplied.contains(&selfish_pkg::entry_id::PARAM_SFO) {
        let title_id = request
            .title_id
            .unwrap_or_else(|| middle_of(request.content_id));
        let title = request.title.unwrap_or(title_id);
        let version = request.version;
        say!("param.sfo: generated for {title_id} ({title:?}, version {version})");
        builder = builder.entry(
            selfish_pkg::entry_id::PARAM_SFO,
            selfish_pkg::sfo::game_bytes(&selfish_pkg::sfo::Params {
                content_id: request.content_id,
                title_id,
                title,
                version,
            }),
        );
    }
    if !supplied.contains(&ICON_ENTRY) {
        say!("icon0.png: none given, using selfish own - supply --entry 0x1200=FILE to replace");
        builder = builder.entry(ICON_ENTRY, crate::icon::default_icon()?);
    }
    if !supplied.contains(&PLAYGO_MANIFEST_ENTRY) {
        say!("playgo-manifest.xml: generated the default manifest");
        builder = builder.entry(PLAYGO_MANIFEST_ENTRY, PLAYGO_MANIFEST.as_bytes().to_vec());
    }
    Ok(builder)
}

/// List every byte range the package left blank because nothing established fills it.
fn report_gaps(built: &selfish_pkg::write::Built) {
    if built.is_complete() {
        say!("no gaps: every byte written is one this crate can account for");
        return;
    }
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

/// The entry ids of `ID=FILE` specs.
pub(crate) fn entry_ids(entries: &[String]) -> Vec<u32> {
    entries
        .iter()
        .filter_map(|spec| spec.split_once('=').map(|(id, _)| id))
        .filter_map(hex_or_dec_u32)
        .collect()
}

/// The title id, the middle field of a content id.
fn middle_of(content_id: &str) -> &str {
    content_id
        .split('-')
        .nth(1)
        .and_then(|part| part.split('_').next())
        .unwrap_or(content_id)
}
