# A third linker script, and the tag that was named but never written


obSCEne now boots healthy on real hardware. Two things here were in the way, and both were the
same shape as the four before them: named in this repository, and never emitted.

**`DT_SCE_ORIGINAL_FILENAME`** sat in the vendor tag table with a comment and no writer. A
shared library is refused without it, and the loader counts what it found - `orig fn 0 mod info
1`. (D079)

**`link/library.ld`** is the third script. `module.ld` and `eboot.ld` differ in exactly two
lines, and a bundled library needs the combination neither of them has. (D078)

### The pattern, three sessions running

Every defect found on hardware in this stretch and the last was a field this repository already
knew the name of. The magic that started this repository (see `CLAUDE.md`) was the same: recorded
one directory away, status *observed from real material*, and not used.

A table row with no writer is not a gap a reader notices. `data/self-format.tsv` and the vendor
tag module are full of rows that are read and never written, and the ones that turn out to be
*required* are indistinguishable from the ones that are optional until a console says so. That is
worth a gate rather than a habit, and it is not one yet.

