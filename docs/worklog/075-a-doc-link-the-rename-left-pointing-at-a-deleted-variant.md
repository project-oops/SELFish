# 2026-09-19 - A doc link the rename left pointing at a variant it deleted, and the gate finally green

Uncovered by [074](074-splitting-title_dir-off-the-too_many_lines-ceiling.md): with clippy clean, the
gate reached its `docs` step and `cargo doc` failed under
`-D rustdoc::broken_intra_doc_links`. `crates/selfish-elf/src/lib.rs` documented `generation()` with a
link to `[`Generation::Previous`]` - a variant that no longer exists. The PS4/PS5 -> Orbis/Prospero
rename left the enum with `Prospero` and `Orbis` and this one doc reference pointing at the old name.

The right target is unambiguous, and not from the prose alone: `generation()` maps `EI_ABIVERSION`
`0` to `Generation::Orbis` (the test at `lib.rs` pins `(0, Some(Generation::Orbis))`), and the
sentence is about that same zero - "the previous generation's own value". Previous generation, abi
zero, `Orbis`. One word, no behaviour, no signature: `Previous` -> `Orbis`.

Pre-existing and committed (last touched in `3299301`, the rename commit), so like 074 it was masked
rather than introduced - it had been red behind clippy, which was red behind fmt. Rustdoc treats a
broken intra-doc link as an error, so this was a genuinely broken deliverable, not a cosmetic one:
the durable product here is the documentation, and a link that resolves to a deleted name is the doc
equivalent of a dangling pointer.

`./bin/selfish check` is now green end to end for the first time in this saga: fmt clean, clippy
clean, 284 tests, docs built. The lesson 073 opened stands - a gate that stops at the first red tells
you about one failure at a time, so "green" is only ever the last red you cleared until you have
watched every step run.
