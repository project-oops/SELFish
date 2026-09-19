# 2026-09-19 - Rustfmt drift a sibling caught, and the gate is still red underneath it

Three committed files were not rustfmt-clean: `crates/selfish-title/src/param.rs` (a method chain
and an `insert(...)` call, both wanting to wrap) and `selfish-cli/src/icon.rs` (an `assert_eq!` that
now fits on one line). `cargo fmt --all` fixes all three; the diff is pure reflow, no semantic change.

## Why it surfaced from outside

Nothing here noticed, because from selfish's own side the gate has not been honestly green in a while
(see [061](061-a-green-gate-that-was-never-checked.md) - the `tail` pipeline that swallowed a red
`check` for twenty-three commits, one of which swept `param.rs` in unintended). The drift rode in on
exactly that.

It was orbistoun that caught it. orbistoun path-depends on `selfish-elf` / `selfish-nid` /
`selfish-container`, so a `cargo fmt --all` run from orbistoun follows the path deps into *this*
whole workspace and checks all of it - including `selfish-title`, which orbistoun does not even use.
orbistoun's `./bin/orbistoun check` went red on our formatting. That is the fix's real point: it
unblocks the sibling's fmt gate.

## The surprise: fmt was not why the gate was red

`./bin/selfish check` is **still red** after this, and not on formatting. `selfish-cli/src/main.rs`
fails clippy `too_many_lines`: `title_dir` is 106 lines against the 100 ceiling (`-D warnings`).
Confirmed pre-existing - it reproduces on the clean committed tree with these fmt changes stashed, in
a file this change never touches. So the gate has been failing on clippy independently of the drift;
formatting was only the part a sibling happened to trip over first. That clippy failure is its own
unit - a `title_dir` that wants splitting, not reformatting - and is left for one, not folded in here.

Gate after this change: `cargo fmt --all -- --check` clean; identity scan clean; `./bin/selfish
check` still red on the pre-existing `too_many_lines` above. No commit.
