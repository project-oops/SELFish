# D084 - A real container can be audited against the format table - the oracle step, as a command


**A real container can be audited against the format table - the oracle step, as a command.**

`data/self-format.tsv` is derived from previous-generation sources and says every row is a
hypothesis until a current-generation file settles it. Until now nothing *ran* that check; it
was a sentence in a header. `selfish_container::audit` and `selfish audit <file>` do it: parse a
real container, identify the generation from the magic, and report which fixed header rows the
file confirms and which it contradicts.

The discipline is in the code, not just the prose. The magic is read to identify the generation
and then **excluded from the tally** - a magic that differs across generations is the split
itself (D003), not a row the newer file got wrong. And a contradiction is reported as a
difference with the real value beside the expected one; it is **not** interpreted. Settling what
a field means at a new generation needs a citable source, not the bytes (principle 1). A
difference is a finding, not a derivation.

Proved by round trip: a container this crate builds confirms every fixed row for both
generations, and a corrupted header byte is reported as differing. On a real current-generation
`eboot.bin` - dumped off a console - the run shows which previous-generation rows carry over,
which is the one thing that turns the table's gen-current rows from hypothesis into measured.

