//! Put the tool's own icon on the Windows executable.
//!
//! One logo, in both places it can appear: `src/icon.rs` embeds it as the `icon0.png` a package
//! gets when the caller supplies none, and this puts the same mark on `selfish.exe` itself.
//!
//! Windows is the only one of the three targets where that is possible. A Linux ELF and a macOS
//! command-line binary have nowhere to carry an icon - theirs live in packaging metadata, a
//! `.desktop` entry or an `.app` bundle, and a bare CLI has neither. So there is nothing to do
//! off Windows, and the dependency is gated in `Cargo.toml` rather than here so it is not fetched
//! or built on those platforms at all.
//!
//! `assets/logo.ico` holds six sizes (16 through 256). Windows picks the one it wants rather than
//! rescaling a single large image, which matters for pixel art: the mark is a blocky drawing, and
//! a filtered downscale of the 256 turns it to mush at 16.
//!
//! It also stamps the commit this was built from into `OOPS_COMMIT`, which `--version` reads. That
//! used to be `oops_build::emit`; it is a few lines of git here instead, so a clone of only this
//! repository builds without reaching outside it. (D104)

use std::path::{Path, PathBuf};
use std::process::Command;

/// The environment variable the build stamps and `--version` reads.
///
/// Named for the collection, not this project, so a CI workflow that exports `OOPS_COMMIT` once
/// stamps every tool in it. A checkout that has never heard of that variable still gets a commit,
/// because [`emit_commit`] falls back to asking git.
const COMMIT_ENV: &str = "OOPS_COMMIT";

fn main() {
    // Which commit this was built from, stamped into `OOPS_COMMIT` for the `--version` line. Asks
    // git when nothing tells it, so there is no configuration to forget.
    emit_commit();

    // The asset is the only input, so a change to it has to trigger a relink. Without this the
    // icon is embedded once and a later change to the logo silently ships the old one.
    println!("cargo:rerun-if-changed=../assets/logo.ico");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../assets/logo.ico");
        // Failing the build is right. A silent fallback would produce a binary that looks correct
        // and is missing the thing this file exists to add, and nobody checks an icon on purpose.
        resource
            .compile()
            .expect("could not embed assets/logo.ico in the executable");
    }
}

/// Stamp the commit this was built from into [`COMMIT_ENV`], for the `--version` line.
///
/// Asks git rather than waiting to be told, so a plain checkout and a clone both get a real commit
/// with nothing to configure. An explicitly supplied `OOPS_COMMIT` still wins, so CI can say so. A
/// modified tree gets `-dirty`, because a binary built from edits is not the commit it would
/// otherwise name, and a stamp pointing at a commit somebody can check out has to be true.
fn emit_commit() {
    // Always watched: this is how CI supplies the commit, and a value that changed without
    // re-stamping would put the previous run's SHA into this run's binary.
    println!("cargo:rerun-if-env-changed={COMMIT_ENV}");
    watch_head();

    if let Ok(supplied) = std::env::var(COMMIT_ENV) {
        let supplied = supplied.trim();
        if !supplied.is_empty() {
            println!("cargo:rustc-env={COMMIT_ENV}={}", shorten(supplied));
            return;
        }
    }

    let Some(short) = git(&["rev-parse", "--short", "HEAD"]) else {
        // No commit, no git, or a repository with no history yet - all ordinary, and all meaning
        // the same thing to a reader: this build has no commit to name.
        return;
    };
    // `--quiet` exits non-zero when there is something to report, so a failure here means the tree
    // is modified rather than that the command did not run.
    let dirty = Command::new("git")
        .args(["diff", "--quiet", "HEAD"])
        .status()
        .is_ok_and(|status| !status.success());
    let suffix = if dirty { "-dirty" } else { "" };
    println!("cargo:rustc-env={COMMIT_ENV}={short}{suffix}");
}

/// Re-run when the commit moves, and not on every build.
///
/// Naming a path that does not exist makes cargo re-run the script on *every* build, so a source
/// tarball with no `.git` would pay a rebuild for a stamp it can never have. The directory is found
/// by walking up rather than hardcoded, because the depth from this crate to the repository root is
/// not fixed.
fn watch_head() {
    let Some(git_dir) = repo_root() else { return };
    let head = git_dir.join("HEAD");
    if head.exists() {
        println!("cargo:rerun-if-changed={}", head.display());
    }
}

/// The nearest `.git` directory at or above this crate, if there is one.
///
/// Reports nothing for a worktree or submodule, where `.git` is a file pointing elsewhere: there is
/// nothing useful to watch then, so watching a file that never changes is worse than watching
/// nothing.
fn repo_root() -> Option<PathBuf> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let mut dir: Option<&Path> = Some(Path::new(&manifest));
    while let Some(here) = dir {
        let candidate = here.join(".git");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if candidate.is_file() {
            return None;
        }
        dir = here.parent();
    }
    None
}

/// One git command, or nothing at all when git is absent or unhappy.
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// A commit as it should be displayed: a hash shortened to seven characters, anything else whole.
///
/// CI hands out the full forty-character SHA, and only plain hex is shortened - a tag or a branch
/// name truncated to seven characters would look like an identifier and identify nothing.
fn shorten(supplied: &str) -> String {
    const DISPLAY_LENGTH: usize = 7;
    let looks_like_a_hash =
        supplied.len() > DISPLAY_LENGTH && supplied.chars().all(|c| c.is_ascii_hexdigit());
    if looks_like_a_hash {
        supplied
            .get(..DISPLAY_LENGTH)
            .unwrap_or(supplied)
            .to_owned()
    } else {
        supplied.to_owned()
    }
}
