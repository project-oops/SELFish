# D029 - The first consumer migrated, and the migration was the best review this repository has had


obSCEne deleted 2,801 lines and took these crates as path dependencies. Four defects surfaced
and **two of them were here**, which is worth stating plainly because the value of a shared
library is usually argued the other way round.

- **A dropped exception.** `dynlib` hardcoded module version 1.1 for every library. obSCEne
  carried a measured exception - the display library at the previous generation is registered
  at 0.0 - and a module declaring 1.1 binds to the current generation's set, which has no way
  to present a frame. The symptom is a module that runs perfectly and draws a black window.
  Now `data/library-versions.tsv` with a row, a provenance header, and a test.
- **A detection that was right about half the problem.** `Tags::detect` decided the convention
  from the string table, reasoning that `5` and `0x6100_0035` cannot be confused. The legacy
  number cannot; `5` is also plain `DT_STRTAB`, so an ordinary shared object read as a
  current-convention vendor module. obSCEne keyed on the vendor-range identity tags, which is
  unambiguous, and that is what this now does. The test that asserted the old behaviour was
  corrected rather than deleted, as in D013.
- **A convention this could not read at all.** `vendor_segment` looks for
  `PT_SCE_DYNLIBDATA`, which a current-convention module does not have - its tables are in a
  `PT_LOAD` and its tags hold addresses. `Elf::tables` now handles both and rebases the
  offsets, so everything in `dynamic` reads either without knowing which. Verified to
  reproduce the old path exactly on all eight real modules.

The fourth was obSCEne's: a stale comment arguing for one `e_type` above code writing the
other, which left with the file.

### And one API that was shaped around one case

`build` took a hash suffix and hashed every name. obSCEne has imports that **are already
identifiers** - firmware exports around a million whose names nobody outside the vendor holds.
It now takes a [`Resolution`] carrying a `Nid`, so hashing a name and decoding an identifier
are the same case, and the suffix parameter is gone.

That is a better shape and it was invisible from inside. A format library with no consumers
cannot tell which of its parameters are facts and which are one caller's habits.

