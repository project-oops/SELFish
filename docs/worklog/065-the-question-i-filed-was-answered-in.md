# 2026-09-10 - The question I filed was answered in a file I had already cited


Worklog 064 moved entry `0x200` into the builder and left `0x1001` - `playgo-chunk.dat` - as the
last thing `--format pkg` asked a caller to supply. This is the entry that moves it, and the part
worth reading is not the port.

## The port, briefly, because it went fine

obSCEne derives the entry in an inline Python block: about twenty-five field writes into a 416-byte
buffer with a sub-table index at `0xC0`, cited to `LibOrbisPkg`'s `ChunkDat.FromProject`. It is now
44 rows in `data/pkg-format.tsv` and a module that writes each row according to its own `type`
column, so the table is the structure rather than a description of one.

Against their built artifact as an oracle: **415 of 416 bytes identical.** Two implementations, two
languages, one cited source, agreeing on every field - including the eight `(offset, size)` pairs
where the last one points at `0x150`, between the fourth and the fifth. That ordering looks like a
mistake and is not; a reader indexes the pairs by position, so tidying it would move
`inner_mchunk_attrs` into whatever slot came fourth.

The 416th byte is the whole entry.

## What I did with the one input I could not account for

`INNER_SIZE=11141120` was a bare literal in their script. `0xAA0000`, and the package containing it
is 7,536,640 bytes - so it could not be a measurement of anything inside that package.

That is correct, and I stopped on it. Writing a number nobody can account for into `data/` is
principle 5 exactly, and the previous day had gone on withdrawing two claims that got in the same
way. So I filed the question outward - measured, arbitrary, or citable? - wrote it up as a blocked
port, and moved on.

**Then I opened `ChunkDat.cs`.** It leaves both sizes zero, each with a comment:

```
size = 0, // must update this to outer pfs image size + pfs offset
size = 0, // must update this to inner pfs image size
```

The field is the inner PFS image. It is named, in the file obSCEne's own comment cites, in a
project this repository's `ACKNOWLEDGEMENTS.md` has listed since the packaging work began. And
`selfish-cli` already had `inner_image_size`, which walks the outer filesystem and reads exactly
that out of the `PFSC` header - written for the cache warning in D071, sitting eleven hundred lines
above the builder that needed it.

I had asked another project a question that two things in my own hands answered.

## The value was the wrong thing, and now it is right

Computed rather than transcribed, this package's inner image is **6,488,064** (`0x630000`), and the
nesting is coherent for the first time:

```
inner PFS image   6,488,064   0x630000
outer image       7,012,352   0x6B0000
package           7,536,640   0x730000
```

`0xAA0000` was larger than the package. The impossibility I noticed was real - I drew the wrong
conclusion from it. **The field was fine; the instance was wrong.**

## What actually shipped

`pack --dir <tree>` with **no `--entry` at all** now writes a complete 14-entry package. The only
gap left is the `0x80` header digest, which is filled later and was never part of this. obSCEne's
`build-pkg.sh` can drop both entry generators, which takes the collection's last Python with them -
their `CLAUDE.md` says there is none, and that block was it.

`inner_image_size` moved into `selfish-pkg` on the way past. Two callers needing one number is a
fact belonging to the library.

## Two things found in passing

**An orphaned doc comment**, the same shape as the `#[command(hide = true)]` that silently hid
`image` from `--help` in worklog 062. `build_image`'s summary - *"Build the image a package
carries, from a tree"* - was sitting above `inner_image_size` with no blank line between them, so
rustdoc rendered one function's documentation opening with another function's first paragraph.
Nothing failed. Moving the function is what surfaced it, because the stranded lines had to go
somewhere.

**A `sed 's/|/\t/g'` over a whole TSV** to convert a heredoc's placeholders hit two pre-existing
rows that contained pipes - `sha256(entry_row || dk3)` and a `signed | encrypted | one unnamed bit`
note. Caught by reading `git diff` for deletions before doing anything else, which is the check
worth keeping: a substitution that is right for the lines you added is not automatically scoped to
them.

## The thing to carry

**A number with no derivation beside it is not the same as a field with no meaning.** I inferred
the second from the first, treated a transcription in a shell script as the state of the evidence,
and filed an ask without reading the source that transcription names.

Before filing outward: read the sources already cited for the thing being asked about. The cost
this time was one request and a few hours, and the answer I found myself was better than the one I
had asked for - it came with a defect in the value rather than a fact about the field. (D099)
