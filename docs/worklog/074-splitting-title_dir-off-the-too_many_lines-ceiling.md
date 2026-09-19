# 2026-09-19 - Splitting title_dir off the too_many_lines ceiling, and a red one layer down

The clippy red that [073](073-rustfmt-drift-a-sibling-caught-and-the-gate-still-red.md) named and
left for its own unit. `selfish-cli/src/main.rs`'s `title_dir` was 106 lines against the 100 ceiling
(`clippy::too_many_lines`, implied by `-D warnings`) - pre-existing, in a file the fmt drift never
touched.

## The cut

`title_dir` lays out a title directory: it copies the caller's `root`, writes `param.json` from the
supplied metadata, normalises the three image assets (icon0/pic0/logo), and then writes three fixed
system files - `keystone`, `pfs-version.dat`, `nptitle.dat`. That last group is the section to lift:
each is generated only if absent, none of the three depends on the caller's metadata, and they are
uniform in shape (`if !exists { generate; write; say "(generated)" }`). It came out verbatim as
`write_generated_system_files(&sce_sys, title_id)` - about thirty lines moved, one call left behind.
Nothing else shifted, the order of the `say!` lines is unchanged, and the 284-test suite is unchanged
and green: the split is a move, not a rewrite.

An `#[allow(clippy::too_many_lines)]` was the other option and the wrong one. The ceiling was doing
its job - it caught a function grown seven distinct write-blocks long - and the fix it asks for, name
a cohesive section and lift it, is the one that leaves the next block somewhere to go instead of one
more entry in a function nobody can hold in their head.

## The surprise: the gate was red one layer deeper

`./bin/selfish check` runs fmt -> lints -> tests -> docs and stops at the first failure (`set -e`),
so a red step hides every step behind it. 073 saw fmt, fixed it, and hit clippy. Fixing clippy here
let the gate reach `docs` for the first time - and `docs` was red too, on a broken intra-doc link in
`selfish-elf`, wholly independent of anything the CLI does. That is
[075](075-a-doc-link-the-rename-left-pointing-at-a-deleted-variant.md). The gate had been failing on
three separate things, each visible only once the one in front of it was cleared.
