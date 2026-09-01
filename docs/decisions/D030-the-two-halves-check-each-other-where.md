# D030 - The two halves check each other where material is unavailable


`Elf::tables`'s current-convention branch was written from reasoning and had never been
executed against anything - every module this repository has been pointed at is
previous-generation. Code that has never run is the defect class this project keeps finding in
other people's work, and shipping one was not defensible.

It cannot be fixed with material nobody has. It can be fixed with the other half of the crate:
`dynlib` can *write* a current-convention module, so the reader can be made to read one.
`tests/current.rs` builds the same linked object under both conventions and asserts that every
tag number differs and every import that comes back out is identical.

That is a real check with a stated limit, and both parts matter. It proves the writer and the
reader agree about the tag numbers, the virtual-address origin, the mapped segment and the
rebasing. It proves nothing about what a console accepts. The doc comment says so, so the
limit travels with the capability rather than living only here.

The same test also pins the one row in `data/library-versions.tsv` end to end: the display
library is declared 0.0 on the previous generation and 1.1 on the current one, read back out of
a built module rather than out of the function that decides it.

