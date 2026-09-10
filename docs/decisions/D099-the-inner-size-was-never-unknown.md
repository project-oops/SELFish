# D099 - The inner size was never unknown, and entry 0x1001 is computed like every other derivable entry

**Status: decided. 2026-09-10.**


**`playgo-chunk.dat` is now built by `selfish-pkg` from `data/pkg-format.tsv`, with no caller
input at all. `--format pkg` and `pack` need no `--entry` for any entry.**

## What was blocking it, and what it actually was

obSCEne's `build-pkg.sh` derives entry `0x1001` in an inline Python block, cited to `LibOrbisPkg`'s
`PlayGo/ChunkDat.cs`. Porting it needs three inputs. Two are computed here already - the content id
is given and the package is `0x80000 + image`. The third was `INNER_SIZE=11141120`, a bare literal
with nothing beside it.

That literal is `0xAA0000`, and **the package containing it is 7,536,640 bytes** - so the value
could not be a measurement of anything inside that package. On that reasoning the port was stopped
and the question filed into obSCEne's inbox (REQ-20260910T0325Z-6b04): measured, arbitrary, or
citable? Writing a number nobody could account for into `data/` is principle 5 exactly, and the day
before had been spent withdrawing two claims that arrived that way.

**The question was already answered, twice, inside this repository's own citations.**

`ChunkDat.FromProject` leaves both mchunk sizes zero with a comment on each:

> `size = 0, // must update this to outer pfs image size + pfs offset`
> `size = 0, // must update this to inner pfs image size`

So the field is `inner_mchunk_attrs[0].size` and it means the inner PFS image - named, in the file
obSCEne's own comment cites, in a project `ACKNOWLEDGEMENTS.md` has listed since the packaging work
began. shadPS4's `playgo_chunk.h` declares the same structure independently and names every field
the same way.

And `selfish-cli` **had a function that computes it**. `inner_image_size` walks the outer
filesystem, finds `pfs_image.dat`, and reads the length its `PFSC` header records - written for the
cache-size warning in D071, sitting eleven hundred lines above the builder it was needed in.

## What was actually wrong, then

Not the field. **obSCEne's value.** SELFish computes this package's inner image as **6,488,064**
(`0x630000`), and the nesting is coherent at last:

```
inner PFS image   6,488,064   0x630000
outer image       7,012,352   0x6B0000
package           7,536,640   0x730000
```

Built from the table against obSCEne's own artifact as an oracle, **415 of 416 bytes are
identical** - two implementations in two languages from one cited source, agreeing on every field
including the sub-table index that is deliberately not in address order. The one byte that differs
is `0x15A`, inside the inner size, and it is the byte their script hardcoded.

## Why the entry is computed rather than supplied

The same argument that moved `0x200` in worklog 064, one step further. Every value in the structure
is fixed for a single-chunk title except those two sizes, and both are in hand at `Builder::build`.
Demanding it from a caller was asking for a fact the builder was holding. A supplied entry still
wins, so a package being rebuilt to match existing material can carry its own.

`inner_image_size` moved into `selfish-pkg` with it. Two callers needing one number is the
definition of a fact belonging to the library rather than to a consumer.

## The rule this is here for

**A number with no derivation beside it is not the same as a field with no meaning, and the second
does not follow from the first.**

The reasoning that stopped the port was sound as far as it went - the value really is impossible,
and refusing to bake it into `data/` was right. What it did not do was ask the next question. It
inferred "the field is unknown" from "this instance of the field is unaccountable", took a
transcription in a shell script as the state of the evidence, and filed a question outward without
first reading the source that transcription names.

**Before filing an ask, read the sources already cited for the thing being asked about.** The cost
here was small and the finding was better than the answer would have been. It will not always be.

This is the fifth instance of one shape: judging what something is from what it is called - a
candidate label, a verdict headline, a field name, a doc comment, a title id in a citation, and now
a constant's variable name. The discriminator each time was going to the material.

## What this has *not* shown, which matters more than usual here

**The corrected value has never been on hardware, and the value it replaces has.**

obSCEne's script records that a header-only version of this entry was refused -
`scePlayGoCoreGetRawContentInfo` returning `0x80f00200` after the header passed - and that the full
structure, `0xAA0000` and all, is what stopped that happening. So `11141120` is a number that
appears in a package a console accepted.

Two readings survive that, and this repository cannot tell them apart from here:

1. **The console does not validate this field against anything**, in which case both values install
   and the correction is a tidiness argument backed by arithmetic.
2. **It does**, in which case the old value was tolerated for some reason and the new one is
   either an improvement or a regression.

The arithmetic is not in doubt - `0xAA0000` is larger than the package that contains it, so it
cannot be describing the inner image, whatever else it is doing. What is in doubt is whether
anything reads it. That is a hardware question, it belongs to obSCEne, and it is filed
(REQ-20260910T0520Z-9c33).

**Recording this rather than leaving it implied**, because the failure mode is specific: a change
justified entirely by reasoning, replacing a value justified entirely by "it worked", is exactly
the shape that gets adopted quietly and blamed six months later for something unrelated. If a probe
comes back refused on this entry, the finding reverses this decision's *value* without touching its
argument - the field would still be the inner PFS image, and `Builder` would gain a way to override
what it measures.

### Answered: reading 1 (2026-09-10)

obSCEne resolved the provenance question (REQ-20260910T0325Z-6b04): `0xAA0000` was **a default
copied from LibOrbisPkg samples during early bring-up, before dynamic inner-image sizing existed,
and the console accepted it without validation.** So reading 1 is the true one - the field is not
checked, both values install, and the correction is the tidiness-backed-by-arithmetic improvement
it looked like, not a regression risk.

That also retired the value for good on their side: obSCEne deleted the hardcoded generator, and
its `make pkg` now builds the entry through this crate - so the number it ships is the measured
`0x630000`, not the old literal.

### Confirmed on hardware (2026-09-10)

The caveat this entry opened with - "the corrected value has never been on hardware" - is now
closed. obSCEne ran a full package sweep on target hardware (REQ-20260910T0520Z-9c33, sweep
`20260910-161904`): a package built with the auto-generated `0x1001` carrying the computed
`0x630000` **installs and launches cleanly**, 54 sections to completion, 174 pass,
`scePlayGoCoreGetRawContentInfo` accepting it without refusal. So the value is no longer justified
by arithmetic alone - it is a measurement of acceptance on the console, the standard this
repository holds a container to. The change is settled in both directions: the old value's
provenance explained, and the new value's acceptance demonstrated.
