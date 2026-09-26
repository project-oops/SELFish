# Decisions

The decisions in force, one file each under `decisions/`. Format and rules are in
[STYLE](https://github.com/project-oops/OOPS/blob/main/docs/STYLE.md#decisions).

**This table is generated.** Edit an entry under `decisions/`, then run
`tools/split-decisions.sh --index selfish`. A number resolves to exactly one file.

| | # | decision | status | date |
|---|---|---|---|---|
| 🟢 | D001 | [One repository for the formats](decisions/D001-one-repository-for-the-formats.md) | decided | 2026-09-26 |
| 🟢 | D002 | [The generation is a type with no default](decisions/D002-the-generation-is-a-type-with-no-default.md) | decided | 2026-09-26 |
| 🟡 | D003 | [Knowledge, not runtime](decisions/D003-knowledge-not-runtime.md) | assumed | 2026-09-26 |
| 🟢 | D004 | [One import hash, pinned by an external fixture](decisions/D004-one-import-hash-pinned-by-an-external-fixture.md) | decided | 2026-09-26 |
| 🟢 | D015 | [`selfish-elf` depends on `selfish-nid`](decisions/D015-selfish-elf-depends-on-selfish-nid.md) | decided | 2026-09-26 |
| 🟢 | D017 | [Relocations are classified here and applied by the consumer](decisions/D017-relocations-are-classified-here-and-applied-by-the-consumer.md) | decided | 2026-09-26 |
| 🟢 | D024 | [Linker scripts are checked against the crate](decisions/D024-linker-scripts-are-checked-against-the-crate.md) | decided | 2026-09-26 |
| 🟢 | D025 | [The module writer takes library resolution from the caller](decisions/D025-the-module-writer-takes-library-resolution-from-the-caller.md) | decided | 2026-09-26 |
| 🟢 | D028 | [The builder states the object type](decisions/D028-the-builder-states-the-object-type.md) | decided | 2026-09-26 |
| 🟢 | D035 | [A derived row ships with the command that re-derives it](decisions/D035-a-derived-row-ships-with-the-command-that-re-derives-it.md) | decided | 2026-09-26 |
| 🟢 | D037 | [The package writer refuses to invent](decisions/D037-the-package-writer-refuses-to-invent.md) | decided | 2026-09-26 |
| 🟢 | D047 | [Never claim to be the vendor](decisions/D047-never-claim-to-be-the-vendor.md) | decided | 2026-09-26 |
| 🟢 | D050 | [`PFSC` is written uncompressed](decisions/D050-pfsc-is-written-uncompressed.md) | decided | 2026-09-26 |
| 🟢 | D054 | [The key blobs are computed from the public keys](decisions/D054-the-key-blobs-are-computed-from-the-public-keys.md) | decided | 2026-09-26 |
| 🟢 | D056 | [The image sits at `0x80000`](decisions/D056-the-image-sits-at-0x80000.md) | decided | 2026-09-26 |
| 🟢 | D062 | [Every crate is listed in CLAUDE.md](decisions/D062-every-crate-is-listed-in-claude-md.md) | decided | 2026-09-26 |
| 🟢 | D071 | [A small inner filesystem is warned about, not adjusted](decisions/D071-a-small-inner-filesystem-is-warned-about.md) | decided | 2026-09-26 |
| 🟢 | D073 | [The CLI owns artwork](decisions/D073-the-cli-owns-artwork.md) | decided | 2026-09-26 |
| 🟢 | D082 | [A current-generation title is a title directory](decisions/D082-a-current-generation-title-is-a-title-directory.md) | decided | 2026-09-26 |
| 🟢 | D084 | [A real container is audited, not transcribed](decisions/D084-a-real-container-is-audited-not-transcribed.md) | decided | 2026-09-26 |
| 🟢 | D089 | [A reverse hash search takes its vocabulary by path](decisions/D089-a-reverse-hash-search-takes-its-vocabulary-by-path.md) | decided | 2026-09-26 |
| 🟢 | D090 | [An unterminated SFO value is a length, not text](decisions/D090-an-unterminated-sfo-value-is-a-length.md) | decided | 2026-09-26 |
| 🟢 | D091 | [A writer refuses where a reader may guess](decisions/D091-a-writer-refuses-where-a-reader-may-guess.md) | decided | 2026-09-26 |
| 🟢 | D096 | [The stated symbol count is not inferred](decisions/D096-the-stated-symbol-count-is-not-inferred.md) | decided | 2026-09-26 |
| 🟢 | D098 | [Measured vendor values are notes on rows](decisions/D098-measured-vendor-values-are-notes-on-rows.md) | decided | 2026-09-26 |
| 🟢 | D099 | [Package entries derivable here are computed](decisions/D099-package-entries-derivable-here-are-computed.md) | decided | 2026-09-26 |
| 🟢 | D100 | [The pipeline is the only way to build](decisions/D100-the-pipeline-is-the-only-way-to-build.md) | decided | 2026-09-26 |
| 🟢 | D101 | [Imports are bound global](decisions/D101-imports-are-bound-global.md) | decided | 2026-09-26 |
| 🟢 | D103 | [The shader container format is built here](decisions/D103-the-shader-container-format-is-built-here.md) | decided | 2026-09-26 |
| 🟢 | D105 | [Linker scripts keep `.eh_frame`](decisions/D105-linker-scripts-keep-eh-frame.md) | decided | 2026-09-26 |

| | meaning |
|---|---|
| 🟢 | settled, and the reasoning rests on something checkable |
| 🟡 | assumed or proposed - made without input, and in the review queue |
| 🔴 | reversed, superseded or blocked |
| ⚪ | no status recorded |

A date with `~` is **not recorded** - it is worked out from the dated entries either
side, because an entry between two of them was written between their dates. `~` alone
is a day both neighbours agree on; `~a..b` is a span, and no day inside it is claimed;
`~>a` and `~<a` are entries with a dated neighbour on only one side. A bare `-` has no
dated entry either side to reason from.
