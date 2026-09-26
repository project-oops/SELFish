//! A title directory: the metadata, artwork and system files around an executable.

use std::path::Path;

use crate::Result;
use crate::pipeline::TitleMeta;

/// The language a title falls back to when it declares only one.
const DEFAULT_LANGUAGE: &str = "en-US";

/// The subtitle when none is given.
const DEFAULT_SUBTITLE: &str = "OOPS Native Title";

/// The category a system-tier title declares, whatever was asked for.
const SYSTEM_CATEGORY: i64 = 0x20000;

/// What a title is, for laying out its directory.
pub(crate) struct Title<'a> {
    pub(crate) id: &'a str,
    pub(crate) name: &'a str,
    pub(crate) category: i64,
    pub(crate) privilege: selfish_container::Privilege,
}

/// Lay out `<out>/<title id>/`: the files under `root`, then `sce_sys` with `param.json`, the
/// artwork and the generated system files.
pub(crate) fn title_dir(
    out: &Path,
    title: &Title<'_>,
    root: Option<&Path>,
    meta: &TitleMeta<'_>,
) -> Result {
    let base = out.join(title.id);
    let sce_sys = base.join("sce_sys");
    std::fs::create_dir_all(&sce_sys)?;
    if let Some(root) = root {
        let copied = copy_tree(root, &base)?;
        say!("{copied} file(s) copied from {}", root.display());
    }
    write_param(&sce_sys, title, meta)?;
    write_artwork(&sce_sys, meta)?;
    write_generated_system_files(&sce_sys, title.id)
}

/// `sce_sys/param.json`.
fn write_param(sce_sys: &Path, title: &Title<'_>, meta: &TitleMeta<'_>) -> Result {
    let mut param = selfish_title::Param::new();
    let category = match title.privilege {
        selfish_container::Privilege::System => SYSTEM_CATEGORY,
        _ => title.category,
    };
    say!("privilege: {:?}", title.privilege);
    param.set_prospero(
        title.id,
        title.name,
        DEFAULT_LANGUAGE,
        category,
        meta.content_id,
        meta.deeplink,
    );
    let sub = meta.subtitle.unwrap_or(DEFAULT_SUBTITLE);
    param.set_title_sub_name(DEFAULT_LANGUAGE, sub);
    say!("subtitle: {sub}");

    if let Some(ver) = meta.version {
        param.set_version(ver);
        param.set_master_version(ver);
        say!("version: {ver}");
    }
    if let Some(cid) = meta.content_id {
        say!("contentId: {cid}");
    }
    if let Some(uri) = meta.deeplink {
        say!("deeplinkUri: {uri}");
    }
    let path = sce_sys.join("param.json");
    std::fs::write(&path, param.to_bytes()?)?;
    say!("{}", path.display());
    Ok(())
}

/// A conversion from a supplied PNG to the form the hardware draws correctly.
type Normalise = fn(&[u8], &str) -> std::result::Result<Vec<u8>, String>;

/// The default a title gets when nothing is supplied.
type Generate = fn() -> std::result::Result<Vec<u8>, String>;

/// `icon0.png`, `pic0.png` and `logo.png`, each supplied and normalised or generated.
///
/// A supplied image is normalised rather than copied: one the hardware does not want is
/// accepted and then drawn wrongly, not refused. (D073)
fn write_artwork(sce_sys: &Path, meta: &TitleMeta<'_>) -> Result {
    let art: [(&str, Option<&Path>, Normalise, Generate, &str); 3] = [
        (
            "icon0.png",
            meta.icon,
            crate::icon::normalise,
            crate::icon::default_icon,
            ", converted to 512x512 RGB",
        ),
        (
            "pic0.png",
            meta.pic0,
            crate::icon::normalise_background,
            crate::icon::default_background,
            "",
        ),
        (
            "logo.png",
            meta.logo,
            crate::icon::normalise_logo,
            crate::icon::default_logo,
            "",
        ),
    ];
    for (name, supplied, normalise, default, converted_note) in art {
        let path = sce_sys.join(name);
        if let Some(from) = supplied {
            let raw = std::fs::read(from)?;
            let converted = normalise(&raw, &from.display().to_string())?;
            std::fs::write(&path, &converted)?;
            let note = if converted.len() == raw.len() {
                ""
            } else {
                converted_note
            };
            say!("{} (from {}{note})", path.display(), from.display());
        } else {
            std::fs::write(&path, default()?)?;
            say!("{} (generated)", path.display());
        }
    }
    Ok(())
}

/// The `sce_sys` files every title carries: a fake-passcode keystone, the pfs version stamp and
/// the NP title descriptor. Each is written only if absent, so one the caller supplied stays.
fn write_generated_system_files(sce_sys: &Path, title_id: &str) -> Result {
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
        std::fs::write(&nptitle_path, nptitle(title_id))?;
        say!("{} (generated)", nptitle_path.display());
    }
    Ok(())
}

/// `nptitle.dat`: the `NPTD` magic, a flag byte, and `<title id>_00` at `0x10`.
fn nptitle(title_id: &str) -> Vec<u8> {
    let mut nptd = vec![0_u8; 160];
    let np_id = format!("{title_id}_00");
    let np_id = np_id
        .as_bytes()
        .get(..np_id.len().min(16))
        .unwrap_or_default();
    selfish_bytes::write_slice(&mut nptd, 0, b"NPTD");
    selfish_bytes::write_slice(&mut nptd, 7, &[0x80]);
    selfish_bytes::write_slice(&mut nptd, 16, np_id);
    nptd
}

/// Copy a directory tree, returning how many files were written.
fn copy_tree(from: &Path, to: &Path) -> Result<usize> {
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
