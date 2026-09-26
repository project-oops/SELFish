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
    write_sound(&sce_sys, meta)?;
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

    let badge = meta
        .badge
        .or_else(|| selfish_title::badge_for_category(category));
    if let Some(badge) = badge {
        param.set_content_badge_type(badge);
        say!("badge: {badge}");
    }

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

/// `icon0.png`, `pic0.png`, `logo.png`, `pic0.dds` and `pic1.dds`.
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

    let dds_art: [(&str, Option<&Path>); 2] =
        [("pic0.dds", meta.pic0_dds), ("pic1.dds", meta.pic1_dds)];
    for (name, supplied) in dds_art {
        let path = sce_sys.join(name);
        if let Some(from) = supplied {
            let raw = std::fs::read(from)?;
            let converted = crate::icon::normalise_dds(&raw, &from.display().to_string())?;
            std::fs::write(&path, &converted)?;
            say!("{} (from {})", path.display(), from.display());
        } else {
            std::fs::write(&path, crate::icon::default_dds())?;
            say!("{} (generated)", path.display());
        }
    }
    Ok(())
}

/// Validate and write `sce_sys/snd0.at9` if supplied or present in root.
fn write_sound(sce_sys: &Path, meta: &TitleMeta<'_>) -> Result {
    let path = sce_sys.join("snd0.at9");
    if let Some(snd0_path) = meta.snd0 {
        let raw = std::fs::read(snd0_path)?;
        let converted = crate::icon::normalise_snd0(&raw, &snd0_path.display().to_string())?;
        std::fs::write(&path, &converted)?;
        say!("{} (from {})", path.display(), snd0_path.display());
    } else if path.exists() {
        let raw = std::fs::read(&path)?;
        let _ = crate::icon::normalise_snd0(&raw, &path.display().to_string())?;
        say!("{} (validated)", path.display());
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "a panic in a test is the test failing"
)]
mod tests {
    use std::path::PathBuf;

    use super::{Title, title_dir};
    use crate::pipeline::TitleMeta;

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "selfish-test-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn title_dir_emits_dds_files_and_paired_badge() {
        let dir = temp_test_dir("title-dds");
        let title = Title {
            id: "TEST00001",
            name: "Test Game",
            category: 0,
            privilege: selfish_container::Privilege::App,
        };
        let meta = TitleMeta {
            content_id: Some("UP0000-TEST00001_00-0000000000000000"),
            version: Some("01.00"),
            icon: None,
            deeplink: None,
            pic0: None,
            pic0_dds: None,
            pic1_dds: None,
            snd0: None,
            badge: None,
            logo: None,
            subtitle: None,
        };

        title_dir(&dir, &title, None, &meta).expect("title_dir succeeds");

        let sce_sys = dir.join("TEST00001").join("sce_sys");
        assert!(sce_sys.join("pic0.dds").exists());
        assert!(sce_sys.join("pic1.dds").exists());
        assert_eq!(
            std::fs::metadata(sce_sys.join("pic0.dds")).unwrap().len(),
            crate::icon::DDS_BC7_TOTAL_SIZE as u64
        );
        assert_eq!(
            std::fs::metadata(sce_sys.join("pic1.dds")).unwrap().len(),
            crate::icon::DDS_BC7_TOTAL_SIZE as u64
        );

        // Verify param.json category 0 paired with badge 1 (Games)
        let param_bytes = std::fs::read(sce_sys.join("param.json")).unwrap();
        let param = selfish_title::Param::parse(&param_bytes).unwrap();
        assert_eq!(param.category(), Some(0));
        assert_eq!(param.content_badge_type(), Some(1));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn title_dir_media_badge_pairing() {
        let dir = temp_test_dir("title-media");
        let title = Title {
            id: "TEST00002",
            name: "Test Media",
            category: 65536,
            privilege: selfish_container::Privilege::App,
        };
        let meta = TitleMeta {
            content_id: None,
            version: None,
            icon: None,
            deeplink: None,
            pic0: None,
            pic0_dds: None,
            pic1_dds: None,
            snd0: None,
            badge: None,
            logo: None,
            subtitle: None,
        };

        title_dir(&dir, &title, None, &meta).expect("title_dir succeeds");

        let sce_sys = dir.join("TEST00002").join("sce_sys");
        let param_bytes = std::fs::read(sce_sys.join("param.json")).unwrap();
        let param = selfish_title::Param::parse(&param_bytes).unwrap();
        assert_eq!(param.category(), Some(65536));
        assert_eq!(param.content_badge_type(), Some(2));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
