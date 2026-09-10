# 2026-09-10 - Half of a refusal was never a refusal


`--format pkg` has been refusing since the pipeline was built, naming two entries it could not
compute:

```
no contents for entries: 0x200 0x1001 - nothing here can compute them, so they must be supplied
```

That refusal has been reported to the operator twice as "the thing blocking the packaging
scripts from retiring", and treated both times as one problem. It was two, and one of them was
this repository's to solve all along.

## `0x200` was a fact the builder was holding

The entry name table is a NUL-separated list of the names of the entries the package carries.
`write.rs` already has `entry_name(id)` mapping every one of them - `param.sfo`,
`playgo-chunk.dat`, `playgo-chunk.sha`, `playgo-manifest.xml`, `icon0.png` - and
`resolve_name_offset` already searches a supplied table for each name to fill in
`entry_record.name_offset`.

So the builder knew every name in the table it was demanding a caller hand it. Requiring it was
asking for a fact it already held.

It is now derived when absent, and a supplied one still wins - a package rebuilt to match
existing material needs its own table byte for byte.

## The order is not sorted, and that is measured

`icon0.png` is `0x1200` and comes **first**, ahead of `0x1000`. The sequence is taken from a
real package via obSCEne's `build-pkg.sh`, where the table is 75 bytes and reads
`\0icon0.png\0param.sfo\0playgo-chunk.dat\0playgo-chunk.sha\0playgo-manifest.xml\0`, and a test
pins it byte for byte and by length.

The format does not *require* that order - `name_offset` points at wherever a name sits, so any
order is self-consistent - but a package built here should look like one that works rather than
merely parse like one. A second test walks every id through `resolve_name_offset` and checks the
offset lands on its own name, because generating a table and then pointing into it wrongly is
worse than not generating one.

## What is left, and it is genuinely the other kind

`0x1001`, `PLAYGO_CHUNK_DAT`, stays supplied. It is a `plgo` structure with a sub-table index at
`0xC0` and chunk, mchunk and scenario records - not a list this crate can assemble from what it
knows. obSCEne derives it from `LibOrbisPkg`'s `ChunkDat.FromProject`, and their own comment
records the cost of guessing at it: a first version wrote only the header, and a console read
the counts, found they promised a chunk the body did not describe, and returned `0x80f00200`
from `scePlayGoCoreGetRawContentInfo` **after the header passed**.

So the refusal now names one entry rather than two, and the one it names is a real gap with a
citable source and a known failure mode.

## The thing worth carrying

**Two facts arriving in the same error message are not the same kind of fact.** These were
grouped because they appeared together in one refusal, and that grouping survived being reported
upward twice without anybody asking whether both halves were true. One was a missing derivation;
the other was a builder declining to look in its own pocket.
