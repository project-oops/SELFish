# 2026-09-10 - One vocabulary, and an attribute that outlived its variant


REQ-20260910T0125Z-5e17 asked for the last of two vocabularies to go: delete `build` rather
than documenting it as superseded, move `stamp` and `wrap` onto `--target`, and bring the
documentation with them.

Done, and the CLI now has one way to say each thing:

- **`build` deleted**, with `BuildTarget` and the three `build_*` functions behind it. The
  previous resolution had left it present-but-superseded with a help notice naming the
  replacement - which was what the earlier request asked for and, under the greenfield rule, is
  itself the cruft.
- **`native` deleted too.** It was a deprecated alias pointing at `selfish build title`, so its
  target no longer existed. Not named in the request; the same rule reaches it.
- **`stamp` and `wrap` take `--target orbis|neo|prospero|trinity`.** Both keep their place for
  the reasons already given - `--library` is the only route to a `.prx`, and `--privilege` and
  `--sdk` have no pipeline spelling - but they no longer speak a second dialect. `stamp` and
  `wrap` now take a `Generation` rather than a `u8`, so the "4 or 5, anything else is a typo"
  conversion disappears from both.

## Which hits were deliberately left

Three of the five references the request flagged are **not the subcommand**: `bin/selfish:7`,
`docs/BUILDING.md:149` and `.github/workflows/release.yml` all say `./bin/selfish build`, which
is the shared workspace verb - a release build through the collection's own entry point. The
request warned about exactly this and it was right to; a string replacement would have broken
the release workflow.

Also left, as the request pre-judged: the `SCE_*_LEGACY` vendor tag names in
`selfish-container`, which are ABI facts, and `data/*.tsv`, where a quoted term is part of a
citation.

## The surprise, and it is a small nasty one

Removing the `Native` variant left its `#[command(hide = true)]` and its doc line behind, and
an attribute does not know which variant it was for. It attached itself to the next one -
`Image` - and **quietly hid a working subcommand from `--help`.**

Nothing failed. The gate passed, the binary built, every test stayed green, and `selfish image`
still ran perfectly if you already knew it was there. It was caught by counting the commands in
`--help` against the ones in the source, which is not a check anybody had reason to run.

That is the day's shape once more: a deletion that removed the code and left the decoration, and
the decoration silently changed the behaviour of its new neighbour. **An attribute is attached
to whatever follows it, so deleting an item and leaving its attributes is not a partial
deletion - it is a redirection.**
