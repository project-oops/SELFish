# 2026-09-09 - Three unnameable imports, and one of them was never a name


Orbistoun filed three import hashes that its own 30,184-name database could not explain, and
asked for names or a statement that there are none. All three got an answer; none of the three
answers was the one the question expected, and that is the entry.

## What was built

`crates/selfish-nid/examples/name_nid.rs` - the reverse direction, as a probe. It takes
identifiers (`0x` and sixteen hex digits, or the eleven characters a symbol name spells) and
vocabulary files **by path**, hashes every word on every line, and reports which reading of the
value a name reproduced. `--suffix <hex|none>` varies the salt for an experiment. The
vocabulary stays out of this repository on purpose: it is obSCEne's mining product, and a
search tool carrying its own word list becomes a stale second copy of one. (D089)

`crates/selfish-nid/tests/depended_on.rs` - a small fixture of identifiers another project has
copied into its own source, so a change here fails here rather than in their build. oops-sdk
asked for three (REQ-20260909T1244Z-716c); its own runtime carries a hasher whose host test
checks only that the output is eleven characters long, which no wrong salt or alphabet can
fail.

## The three surprises

**A published name that does not hash to its own identifier.** obSCEne's mined table attributes
the first identifier to `sceAgcInit` in `libSceAgc` from five independent sources - and
`Nid::of("sceAgcInit")` is a different value, which the same table *also* carries against the
same name. Not a typo: 108 of 116 published `libSceAgc` identifiers reproduce from their name,
8 do not, and five of those are the second identifier on a name whose first one reproduces.
Decoration sweeps and the empty suffix were both ruled out by running them. So the attribution
is somebody else's evidence rather than a derivation, and no name search can ever reach it -
which is the answer to why the asking project's search failed rather than an excuse for it.
The semantics corroborate: three titles that import it all stall inside that library's
initialisation, which is what a stalled `sceAgcInit` looks like.

**A bound is a better answer than a shrug.** The second identifier is named by nobody in either
byte order, across 166,971 mined names and 1,130,757 identifiers observed without one. The
useful part is that the same corpus publishes `Func_<HEX>` placeholders for identifiers seen but
unnamed - and this one is not among *those* either, so no public table has even observed it.

**The third was never a name.** It is the hash of the literal string `$fYZQG4CU71c`, and the
leading `$` is the consumer's own sigil for "this name *is* the identifier". Something hashed a
string that was already an identifier. The result belongs to no library and can never be named,
which is exactly the reported symptom - an import that "resolves to no library name at all".

That one generalises, and it is why D089 exists. **An unnameable hash is evidence about a
toolchain before it is evidence about a vocabulary**, and the forward direction settles it in a
single run: hash the candidate string and see whether the mystery value comes back.

Two things checked while answering it, both reassuring:

- `selfish imports` on the consumer's **module** leg shows `fYZQG4CU71c` against `libSceAgc`.
  `mkmodule` honours the sigil; the defect is not here.
- `selfish elf` on its **payload** leg reports `0 vendor import entries, 0 DT_NEEDED`. So the
  question "which library ordinal does that import entry carry" has no ordinal to answer with -
  there is no vendor import table in that file at all, and a reader walking one is reading a
  plain ELF as though it were a vendor module.

## Also learned

`sceKernelGetProcessId` appears nowhere in a 166,971-name corpus mined from eight emulator
projects. `getpid` does, from ten sources, against `libkernel` and `libScePosix`. A correct
hash of a name the platform does not export resolves to nothing just as surely as a wrong hash
of one it does, so that went back with the value rather than after it.
