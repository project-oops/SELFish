# D033 - The six unknown package entries all vary between titles, so there is no constant to fall back on


"Six of fourteen entries have no established meaning" has been the whole answer since D012, and
it left one obvious workaround open: if an unknown entry were the same in every package, a
writer could emit that constant and be no more wrong than the tooling that produced them.

Measured across three unrelated titles with `examples/entries.rs`:

```
of the 14 entries a writer must produce:
  8 have a cited meaning
  0 are byte-identical across all 3 titles - boilerplate
  6 differ between titles: 0x1, 0x80, 0x100, 0x400, 0x401, 0x1002
```

**Zero boilerplate.** Every unknown entry encodes something about the title it belongs to, so
there is nothing to copy even if copying were allowed - and it is not, since taking bytes out
of somebody's package is derivation-from-material rather than from a source (principle 2).

This makes the block firmer than it was and worth stating that way round: it is no longer
"nobody has looked hard enough", it is "the missing information is per-title and nothing here
can compute it". What unblocks it is unchanged and is a *source*: a packaging tool whose code
says what goes in those six.

Recorded because the shortcut is the first thing anyone will think of, and now the answer to it
is a measurement rather than an opinion.

### The first version of this measurement was wrong

It reported nine entries as boilerplate. Entries present in only one package were compared
against themselves - a one-element window, and `all()` over an empty iterator is true - so
every singleton counted as identical. Restricting the count to the fourteen a writer must
actually produce fixed it.

A measurement that flatters the conclusion you were hoping for is worth re-reading before
quoting. This one said the block might be soft; it is not.

