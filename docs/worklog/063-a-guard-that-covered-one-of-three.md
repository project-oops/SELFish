# 2026-09-10 - A guard that covered one of three, and five copies of one table


REQ-20260910T0015Z-8e44, the documentation audit that was not the CLI vocabulary. Most of it
was prose catching up with code, and two items were real.

## The guard was a third of what its own docs claimed

`./bin/selfish links` exists for one reason, stated in `docs/BUILDING.md`: prove the linker
tests **ran** rather than skipping for a missing `ld.lld`, because a skip would pass having
verified none of the segment layout or tag conventions.

It ran `cargo test -p selfish-elf --test links` and nothing else. Three test targets link a
module against `link/module.ld` - `links`, `current` and `module` - so **two of the three could
skip silently under the very verb that exists to stop that.**

Now a loop over all three, failing on the first skip and naming which target. Verified by
running it here, where `ld.lld` is absent: it stops at `links`, says so, exits 1. The guard
refusing is the guard working.

The prose was wrong in the other direction too - "two integration tests" where there are three
files - and per the convention the fix is to remove the number rather than write today's. Both
copies now describe the shape: *the integration tests that link*.

## Five copies of one table, two already drifted

The category table - `applicationCategoryType`, the DMEM budget and display ownership - existed
in `writing.md`, this repo's `GLOSSARY.md`, the meta glossary, `param.rs` and the CLI. The two
prose copies now point at the collection's four-axes table and at `selfish_title::category`,
which is what code actually reads.

Kept in place at both sites: the one consequence people hit, which is that a GUI under
`system-app` gets zero direct memory and `sceVideoOutOpen` fails `0x80290001` - the app starts
and cannot draw. A cross-link that drops the thing somebody needed at the point they needed it
is not a simplification.

The "oracle, never a source" rule was stated four times; `README.md` already deferred to
`CLAUDE.md`, and the other two now say which statement is canonical rather than restating it
independently.

## What was deliberately not touched, and why that mattered

The request opened with the exclusions, which was the right order. Confirmed untouched:

- **`SCE_*_LEGACY` in `crates/selfish-elf`** - 13 references, all intact. These are dynamic tag
  *numbers* a loader matches on, and `dynamic.rs` pairs each with a `*_CURRENT` sibling, so a
  half-rename breaks `Tags::detect` - which this repository's own `CLAUDE.md` records as having
  been wrong once already.
- **`data/*.tsv`, `pkg-keys.toml`, `sdk-versions.toml`** - no changes at all. Quoted terms
  inside provenance citations; `OpenOrbis-PS4-Toolchain` is a project name, not vocabulary.
- The generated indexes, which are rebuilt from per-entry files.

`git diff --stat crates/ data/` was empty at the end, which is the check rather than the
intention.

## The shape of it

Three of the audit's findings were prose describing behaviour that never existed - `extract
--out`, positional symbol names for `sections`, `--all` as a filter when it is a truncation.
Each would send a reader to an error rather than to an out-of-date flag, which is the worse
failure: a stale name looks stale, and a confident wrong description does not.
