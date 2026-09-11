# D104 - selfish-cli depends on nothing outside this repository again: the commit stamp is local git, the logging was dead weight

**Status: decided. 2026-09-11.**

The libraries here depend on nothing outside this repository, and that is the property the
repository exists to hold - it is what lets orbistoun, obSCEne and prosperous take these formats
without inheriting a build. `selfish-cli` had quietly given it up: it took `oops-build` and
`oops-log` from the sibling `oops-libs` by relative path, for a git-commit stamp on `--version`
and a logging subscriber, so a clone of only SELFish did not build without fetching a sibling
first. BUILDING.md recorded this as a regression rather than papering over it; this closes it.

## What the two dependencies were, and what happened to each

**`oops-build` - reimplemented locally.** Its job is a build-time stamp: `emit()` writes the
short commit into `OOPS_COMMIT` (asking git, or taking a CI override), and `line!()` reads it
back into clap's `--version` as `v{version} - {commit}[-dirty]`. That is a few lines of git and
`option_env!`, and it now lives in `selfish-cli/build.rs` (`emit_commit`) and `main.rs`
(`version_line`). The env var stays `OOPS_COMMIT`, not a selfish-specific name, so a CI workflow
that exports it once still stamps this tool. `--version` is unchanged: `selfish v0.1.0 -
<sha>[-dirty]`.

`oops_build::line!()` had to be a macro because `env!`/`option_env!` in a shared crate would
read *that* crate's version and compilation environment, not the consumer's. Reimplemented
*inside* selfish-cli, the same two macros expand at the right call site, so `version_line` is a
plain function - the reason for the macro was the crate boundary, and the boundary is gone.

**`oops-log` - removed, not reimplemented.** It installs a `tracing` subscriber. Nothing in
these crates emits a tracing event - no `info!`, no `warn!`, no `#[instrument]`, and the library
crates take no dependency on `tracing` at all - so the subscriber wrapped nothing. A local
reimplementation would have been faithful to a feature that does nothing. It and `tracing` are
gone; the CLI's diagnostics go through the local `say!` macro to stdout, as they always did.

## What did shrink, deliberately

The reimplemented stamp is a touch smaller than `oops_build`: a build with no commit at all - a
source tarball unpacked outside a repository - prints `v{version} - no commit` rather than
falling back to the executable's build time. That fallback needed a civil-date conversion this
tool has no other use for, and `no commit` is the honest answer for the case it covers. In a
checkout or in CI the stamp is a real commit, which is every build that matters.

CI drops its bootstrap step for this project, because there is now nothing to bootstrap. The
crates under `crates/` never depended on oops-libs; now neither does the CLI, so nor does the
workspace.
