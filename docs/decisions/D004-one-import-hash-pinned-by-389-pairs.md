# D004 - One import hash, pinned by 389 pairs somebody else produced


The hash existed twice - in an emulator and in a probe - and the probe's header said the
duplication was *deliberate*:

> Two implementations are deliberate - this tool must not depend on the emulator it measures

The reasoning is sound and worth stating properly, because it is not the usual
don't-repeat-yourself argument. A probe works by importing tens of thousands of symbols by
hash and reporting which resolve. If the probe and the loader share a hash implementation,
"it resolved" proves only that both did the same thing - a wrong hash would agree with itself
and the report would say everything is fine.

**Overruled, and the fixture is why that is defensible rather than merely decided.**
`tests/known-pairs.txt` carries 389 name-and-encoding pairs harvested from the resolution logs
of open-source emulators: each line is a case where an independent implementation hashed a
name, matched it, and printed what it matched. Reproducing all 389 constrains the suffix, the
byte order, the alphabet and the packing simultaneously, against implementations nobody here
consulted.

Two of our own implementations agreeing is evidence about us. Agreeing with 389 pairs produced
elsewhere is evidence about the algorithm. The fixture answers the objection better than the
duplication did - and notably, the probe's own note records that the one time the byte order
was wrong, it went unnoticed *because neither implementation had a fixture*. The duplication
never caught it. A fixture would have.

Both directions are checked, and the count is asserted: a fixture file that silently stopped
being read would otherwise make the test pass while proving nothing, which is exactly the
failure this crate is written against.

Status: **decided**.

