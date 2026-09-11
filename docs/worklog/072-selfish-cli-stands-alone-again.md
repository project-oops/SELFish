# 2026-09-11 - selfish-cli stands alone again: the local commit stamp, and the logging that logged nothing

Took the oops-libs cleanup BUILDING.md had been flagging as a regression: `selfish-cli` depended
on `oops-build` and `oops-log` from the sibling `oops-libs`, so a clone of only SELFish did not
build. Both dependencies are gone (D104).

**The commit stamp came home in a few lines.** `oops_build::emit()` → `emit_commit()` in
`build.rs` (git short SHA into `OOPS_COMMIT`, a CI override winning, `-dirty` when the tree is
modified); `oops_build::line!()` → `version_line()` in `main.rs`. The macro was only a macro
because of the crate boundary - `env!`/`option_env!` have to expand at the consumer's call site -
and inside selfish-cli that boundary is gone, so it is a plain function now. `--version` is
byte-for-byte what it was: `selfish v0.1.0 - <sha>-dirty`.

**The surprise: the logging logged nothing.** `oops-log` installs a `tracing` subscriber, and I
went in expecting to reimplement it. Grep found no `tracing::` event anywhere in the workspace,
and no library crate depends on `tracing` at all - the subscriber had been wrapping a facade
nobody called. So it is deleted, not reimplemented, and `tracing` left with it. `Cargo.lock` shed
162 lines: oops-build, oops-log, tracing, tracing-core, tracing-subscriber and their transitive
deps.

**Docs and CI followed the code.** README, BUILDING, the library feature doc, the `bin/selfish`
header and the CI workflow all described the dependency as a live regression; all now say the
property holds, and CI drops its bootstrap step because there is nothing left to bootstrap.

**One circumstance worth recording:** this landed on top of a concurrent session's in-flight
PS4/PS5 → Orbis/Prospero rename, at the operator's direction - the two changes share a commit
rather than the cleanup waiting for the working tree to clear. The rename touches
`selfish-container`, `selfish-title` and `data/sdk-versions.toml`; the cleanup touches
`selfish-cli`, the workspace manifest and the docs, so they do not overlap in substance.
