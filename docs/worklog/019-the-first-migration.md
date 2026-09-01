# The first migration


obSCEne now depends on these crates. It deleted `nid.rs`, `dynlib.rs`, `module.rs` and
`mkself.rs` - 2,801 lines - along with three `data/*.tsv` snapshots and its copy of
`link/module.ld`. Its `data/` now holds only its own measurements, which is the split this
repository exists to make.

Done incrementally with a build after every step, because obSCEne had **no version control at
all** - `.gitignore`, `.gitattributes` and `.github/` but no `.git`. It was initialised and
staged first, so there was something to fall back to.

### The verification that mattered

obSCEne re-derives the vendor tag assignment from the bytes it just wrote, and that checker
knows nothing about how they were produced:

```
p.elf: EI_OSABI 0x0000 -> 0x0009
p.elf: e_type 0x0003 -> 0xfe10
p.elf: vendor segment at 0x297e0, 408 bytes, 23 tags, 1 symbols from 1 libraries
p.elf: layout reproduces the tag derivation (9 relations)
```

Its published-pair self-test also still passes, through the suffix that moved here - which is
the tightest possible check that a constant survived a move.

### Four defects, two of them ours

Recorded in D029. The short version: this repository had dropped a measured version exception,
detected the tag convention with a test that matched every ordinary ELF, and could not find
the tables under the current convention at all. obSCEne had all three right.

Also `build`'s signature was shaped around the only case its author had in hand. It hashed
every symbol name; obSCEne has imports that arrive *as* identifiers with no name behind them.
It now takes a `Resolution`, and the hash suffix is no longer a parameter at all.

**A format library with no consumers cannot tell which of its parameters are facts and which
are one caller's habits.** That is the argument for migrating the second one sooner rather
than later.

