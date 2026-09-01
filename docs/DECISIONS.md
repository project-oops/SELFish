# Decisions

Numbered, with reasoning, as they are made. The reasoning is the point - it is what stops a
choice being re-litigated by somebody who only has the choice.

**None of D001-D066 carries a date, and none is being invented.** The oldest file in the tree
is stamped 2026-08-26, so the whole log was written across two days - that bound is checkable
and a per-entry date would not be. Entries from D067 on carry one.

**This table is generated.** Edit an entry under `decisions/`, then run
`tools/split-decisions.sh --index selfish`. A number resolves to exactly one file.

| | # | decision | status | date |
|---|---|---|---|---|
| ⚪ | D001 | [A fourth repository, holding formats and nothing that knows what a consumer is for](decisions/D001-a-fourth-repository-holding-formats-and.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D002 | [The generation is a type with no `Default`](decisions/D002-the-generation-is-a-type-with-no-default.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D003 | ["Not an SDK" was the wrong line. The line is knowledge against runtime](decisions/D003-not-an-sdk-was-the-wrong-line-the-line.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D004 | [One import hash, pinned by 389 pairs somebody else produced](decisions/D004-one-import-hash-pinned-by-389-pairs.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D005 | [OOPS is additive, never required](decisions/D005-oops-is-additive-never-required.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D006 | [`Entry` answers for itself, because the first version of it could not be called](decisions/D006-entry-answers-for-itself-because-the.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D007 | [The container is byte-identical to the implementation it replaces](decisions/D007-the-container-is-byte-identical-to-the.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D008 | [The package's outer layer first, because it needs no cryptography](decisions/D008-the-package-s-outer-layer-first-because.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D009 | [The crypto chain is unchanged between generations, and that is a finding](decisions/D009-the-crypto-chain-is-unchanged-between.md) | unrecorded | ~<2026-08-29 |
| 🟢 | D010 | [Every container in three current-generation packages uses the *previous* generation's magic](decisions/D010-every-container-in-three-current.md) | measured | ~<2026-08-29 |
| 🟢 | D011 | [The image offset is a header field, and the evidence that it was not could not have failed](decisions/D011-the-image-offset-is-a-header-field-and.md) | confirmed | ~<2026-08-29 |
| ⚪ | D012 | [What a package writer still needs, and what it does not](decisions/D012-what-a-package-writer-still-needs-and.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D013 | [Two vendor tag ranges, and each side documented only the one it uses](decisions/D013-two-vendor-tag-ranges-and-each-side.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D014 | [Reassembly is the inverse of building, and both live together](decisions/D014-reassembly-is-the-inverse-of-building.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D015 | [`selfish-elf` depends on `selfish-nid`, which reorders the spine](decisions/D015-selfish-elf-depends-on-selfish-nid.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D016 | [An import's library is looked up by its id, not by its position in the table](decisions/D016-an-import-s-library-is-looked-up-by-its.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D017 | [Relocations are read and censused here; applying them stays with the consumer](decisions/D017-relocations-are-read-and-censused-here.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D018 | [An unrecognised relocation type gets no name](decisions/D018-an-unrecognised-relocation-type-gets-no.md) | unrecorded | ~<2026-08-29 |
| 🟢 | D019 | [The `PARAM.SFO` alignment rule came from a source and was refuted by eleven files](decisions/D019-the-param-sfo-alignment-rule-came-from.md) | derived | ~<2026-08-29 |
| 🟢 | D020 | [Unterminated text is a separate value variant, not a flag](decisions/D020-unterminated-text-is-a-separate-value.md) | derived | ~<2026-08-29 |
| ⚪ | D021 | [There is a second package magic, and it is named and refused rather than guessed at](decisions/D021-there-is-a-second-package-magic-and-it.md) | unrecorded | ~<2026-08-29 |
| 🔴 | D022 | [Package writing stays blocked, and the source that looked like it might unblock it does not](decisions/D022-package-writing-stays-blocked-and-the.md) | blocked | ~<2026-08-29 |
| ⚪ | D023 | [Sections are a separate module from `dynamic`, because they answer a different question](decisions/D023-sections-are-a-separate-module-from.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D024 | [The linker script moves here, and its constants are tested against the crate's](decisions/D024-the-linker-script-moves-here-and-its.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D025 | [The dynamic-table writer moves here, and the manifest stays with the caller](decisions/D025-the-dynamic-table-writer-moves-here-and.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D026 | [The end-to-end test is the point of putting both halves in one crate](decisions/D026-the-end-to-end-test-is-the-point-of.md) | unrecorded | ~<2026-08-29 |
| 🔴 | D027 | [Filesystem writing is blocked, and now measured rather than asserted](decisions/D027-filesystem-writing-is-blocked-and-now.md) | blocked | ~<2026-08-29 |
| ⚪ | D028 | [The header identity is stamped here too, and `e_type` is a parameter rather than a constant](decisions/D028-the-header-identity-is-stamped-here-too.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D029 | [The first consumer migrated, and the migration was the best review this repository has had](decisions/D029-the-first-consumer-migrated-and-the.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D030 | [The two halves check each other where material is unavailable](decisions/D030-the-two-halves-check-each-other-where.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D031 | [The command-line tool does not panic when a pipe closes](decisions/D031-the-command-line-tool-does-not-panic.md) | unrecorded | ~<2026-08-29 |
| 🟢 | D032 | [This repository gets a CI workflow, and obSCEne's is marked as unable to pass](decisions/D032-this-repository-gets-a-ci-workflow-and.md) | decided | ~<2026-08-29 |
| ⚪ | D033 | [The six unknown package entries all vary between titles, so there is no constant to fall back on](decisions/D033-the-six-unknown-package-entries-all.md) | unrecorded | ~<2026-08-29 |
| 🟢 | D034 | [Two package entries were derived rather than cited, and there is a command that re-derives them](decisions/D034-two-package-entries-were-derived-rather.md) | derived | ~<2026-08-29 |
| 🟢 | D035 | [A derivation ships with the command that re-runs it](decisions/D035-a-derivation-ships-with-the-command.md) | derived | ~<2026-08-29 |
| ⚪ | D036 | [Entry `0x80` is a digest manifest over named things, and two of its slots are established](decisions/D036-entry-0x80-is-a-digest-manifest-over.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D037 | [A package writer that refuses to invent the entries nothing established explains](decisions/D037-a-package-writer-that-refuses-to-invent.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D038 | [What the cryptographic entries are not, recorded so nobody re-runs the searches](decisions/D038-what-the-cryptographic-entries-are-not.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D039 | [LibOrbisPkg named all six, and the derivations were right](decisions/D039-liborbispkg-named-all-six-and-the.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D040 | [`PLAYGO_CHUNK_SHA` is solved, and material settled what the source left open](decisions/D040-playgo-chunk-sha-is-solved-and-material.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D041 | [The licence entries are stored encrypted, so naming them was not enough](decisions/D041-the-licence-entries-are-stored.md) | unrecorded | ~<2026-08-29 |
| 🟢 | D042 | [The filesystem superblock was never a wall - it was 95 unnamed bytes](decisions/D042-the-filesystem-superblock-was-never-a.md) | measured | ~<2026-08-29 |
| ⚪ | D043 | [Naming a field is not the same as being able to write one](decisions/D043-naming-a-field-is-not-the-same-as-being.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D044 | [The licence entries were never unknown - a thirty-two-byte record, not three fields](decisions/D044-the-licence-entries-were-never-unknown.md) | unrecorded | ~<2026-08-29 |
| 🟢 | D045 | [Understood is not written](decisions/D045-understood-is-not-written.md) | derived | ~<2026-08-29 |
| 🟢 | D046 | [The licence structure is measured; producing one needs two keys this repository does not hold](decisions/D046-the-licence-structure-is-measured.md) | measured | ~<2026-08-29 |
| 🟢 | D047 | [A licence is built from scratch, and a real one is reproduced byte for byte](decisions/D047-a-licence-is-built-from-scratch-and-a.md) | derived | ~<2026-08-29 |
| ⚪ | D048 | [`pack` now demands only what a title is, not what a package is](decisions/D048-pack-now-demands-only-what-a-title-is.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D049 | [The filesystem is built in two halves, because a package has two filesystems built to different rules](decisions/D049-the-filesystem-is-built-in-two-halves.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D050 | [`PFSC` does not compress, and that is the format rather than a shortcut](decisions/D050-pfsc-does-not-compress-and-that-is-the.md) | unrecorded | ~<2026-08-29 |
| 🟢 | D051 | [The two ways of writing a `1` near the end of the superblock are two fields, not one](decisions/D051-the-two-ways-of-writing-a-1-near-the.md) | measured | ~<2026-08-29 |
| ⚪ | D052 | [A block signature is an HMAC under a key the builder computes](decisions/D052-a-block-signature-is-an-hmac-under-a.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D053 | [Signature ordering is the correctness argument, so it is written as one sequence](decisions/D053-signature-ordering-is-the-correctness.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D054 | [The key blobs are computed, and the proof is that they come out byte-identical to real packages](decisions/D054-the-key-blobs-are-computed-and-the.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D055 | [The passcode reaches further than the key blobs, and a test keyed a package differently to find out](decisions/D055-the-passcode-reaches-further-than-the.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D056 | [The header past `0x410` was entirely zero, so nothing could be mounted](decisions/D056-the-header-past-0x410-was-entirely-zero.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D057 | [The flat path table is real now, and it was built precisely because nothing here reads it](decisions/D057-the-flat-path-table-is-real-now-and-it.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D058 | [`selfish image` exists because another session asked for it in a script comment](decisions/D058-selfish-image-exists-because-another.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D059 | [A `param.sfo` is a format and belongs here. An icon is a picture and does not](decisions/D059-a-param-sfo-is-a-format-and-belongs.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D060 | [The default icon is selfish's own mark, reversing D059](decisions/D060-the-default-icon-is-selfish-s-own-mark.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D061 | [The `param.sfo` field set was the previous generation's, and two homebrew packages said so](decisions/D061-the-param-sfo-field-set-was-the.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D062 | [A second `PARAM.SFO` implementation, and why it happened](decisions/D062-a-second-param-sfo-implementation-and.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D063 | [One directory-entry writer, because the copy had the rule without the reasoning](decisions/D063-one-directory-entry-writer-because-the.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D064 | [A literal NUL byte in a source file made it invisible to `grep`, which hid a duplicate constant](decisions/D064-a-literal-nul-byte-in-a-source-file.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D065 | [`cargo doc` was failing, and the gate did not build docs](decisions/D065-cargo-doc-was-failing-and-the-gate-did.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D066 | [Documentation drifts in the direction of claiming less than the code does](decisions/D066-documentation-drifts-in-the-direction.md) | unrecorded | ~<2026-08-29 |
| 🟢 | D067 | [Supporting the current generation is three-quarters done and blocked on one oracle](decisions/D067-supporting-the-current-generation-is.md) | done | ~<2026-08-29 |
| ⚪ | D068 | [The logo is one file, committed, and the drawn mark is gone](decisions/D068-the-logo-is-one-file-committed-and-the.md) | unrecorded | ~<2026-08-29 |
| 🟢 | D070 | [The cache size is a ceiling measured from the image, not a constant](decisions/D070-the-cache-size-is-a-ceiling-measured.md) | measured | ~<2026-08-29 |
| ⚪ | D071 | [D070's rule was wrong, and the corrected one cannot be tested by avoiding it](decisions/D071-d070-s-rule-was-wrong-and-the-corrected.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D072 | [The inner filesystem's root pointed its parent at the super root](decisions/D072-the-inner-filesystem-s-root-pointed-its.md) | unrecorded | ~<2026-08-29 |
| ⚪ | D073 | [The tool converts a supplied icon, rather than asking four projects to export one](decisions/D073-the-tool-converts-a-supplied-icon.md) | unrecorded | ~<2026-08-29 |
| 🟢 | D074 | [An executable declares no export library, which frees library id zero](decisions/D074-an-executable-declares-no-export.md) | decided | 2026-08-29 |
| 🟢 | D075 | [A container entry's `memsz` is its data's size, not the segment's memory size](decisions/D075-a-container-entry-s-memsz-is-its-data-s.md) | decided | 2026-08-29 |
| 🟢 | D076 | [The dynamic table lives at the tail of the vendor segment, not in the image](decisions/D076-the-dynamic-table-lives-at-the-tail-of.md) | decided | 2026-08-29 |
| 🟢 | D077 | [The vendor segment begins with a fingerprint region, and leaving it out moves everything](decisions/D077-the-vendor-segment-begins-with-a.md) | decided | 2026-08-29 |
| ⚪ | D078 | [A bundled library is a third layout, and it needs a third linker script](decisions/D078-a-bundled-library-is-a-third-layout-and.md) | unrecorded | ~>2026-08-29 |
| ⚪ | D079 | [`DT_SCE_ORIGINAL_FILENAME` is required, and holds the module's own name](decisions/D079-dt-sce-original-filename-is-required.md) | unrecorded | ~>2026-08-29 |
| 🟢 | D080 | [The keystone is derived, not supplied, and every package this crate built was missing one](decisions/D080-the-keystone-is-derived-not-supplied.md) | derived | ~>2026-08-29 |
| ⚪ | D081 | [`param.json` does not belong in a package, and its presence there was mixing two routes](decisions/D081-param-json-does-not-belong-in-a-package.md) | unrecorded | ~>2026-08-29 |
| ⚪ | D082 | [A package cannot produce a current-generation title, so `native` is a second delivery route rather than a second package format](decisions/D082-a-package-cannot-produce-a-current.md) | unrecorded | ~>2026-08-29 |
| ⚪ | D083 | [A comment in the keyset file broke the licence, because the reader matched prose](decisions/D083-a-comment-in-the-keyset-file-broke-the.md) | unrecorded | ~>2026-08-29 |
| 🟢 | D084 | [A real container can be audited against the format table - the oracle step, as a command](decisions/D084-a-real-container-can-be-audited-against.md) | derived | ~>2026-08-29 |
| ⚪ | D085 | [Getting a real current-generation SELF is a measurement obSCEne runs, not a file this repo holds - and the sandbox makes it conditional](decisions/D085-getting-a-real-current-generation-self.md) | unrecorded | ~>2026-08-29 |
| ⚪ | D086 | [Confirm the format on the console and report the verdict - do not carry the bytes off](decisions/D086-confirm-the-format-on-the-console-and.md) | unrecorded | ~>2026-08-29 |
| ⚪ | D087 | [Native PS5 title manifests (`param.json`) and differential measurement over fake-signed packaging](decisions/D087-native-ps5-title-manifests-param-json.md) | unrecorded | ~>2026-08-29 |
| 🔴 | D088 | [`libkernel_vaddrs` example: exports as `name vaddr`, and why the names are not ours](decisions/D088-libkernel-vaddrs-example-exports-as.md) | superseded | ~>2026-08-29 |

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
