# D089 - Naming a hash backwards is a search over somebody else's vocabulary, so the vocabulary is an argument

**Status: decided. 2026-09-09.**


**Naming a hash backwards is a search over somebody else's vocabulary, so the vocabulary is
an argument rather than a dependency.**

Orbistoun asked this repository to name three identifiers that a running guest imports and
that its own 30,184-name database cannot explain (REQ-20260909T1250Z-1f74). The forward
direction has lived here since D004: hash a name, get an identifier. The reverse has no closed
form - the hash is one-way - so the only method is to hash a vocabulary and look for the value,
and whichever vocabulary you hash *is* the answer's strength.

That is exactly the shape the admission test forbids owning. A million-odd mined identifiers
are obSCEne's measurement product and stay there; a search tool that shipped its own word list
would be a second copy of that corpus wearing a different name, and would drift from it.

So `crates/selfish-nid/examples/name_nid.rs` takes the vocabulary **by path** and hashes every
whitespace-separated word on every line, which means a table with the name in any column works
without being reshaped first. What the repository keeps is the format knowledge: the hash, the
encoding, and the two things a caller gets wrong.

## The two things a caller gets wrong, and why the tool prints both

**Byte order.** An identifier read one way and the same identifier read the other are both
plausible sixteen-hex-digit numbers, and this collection contains three conventions that
disagree: `selfish` and obSCEne's mined table write the little-endian `u64` that `Nid::value`
returns, and orbistoun prints it byte-reversed. Nothing about either form says which it is. So
a raw value is searched **both ways** and the output names the reading that hit.

**The encoded form.** A corpus that stores eleven characters cannot be grepped for a hex value
at all. The tool prints the encoding of every reading whether or not a vocabulary was supplied,
because that string is what makes the question askable of a corpus in the other notation.

`--suffix` is there for the same reason `Nid::with_suffix` is: an identifier that no name
explains is one of the few honest reasons to ask whether the salt is what differs, and asking
costs one run.

## What the search found, which is the part worth keeping

Three answers of three different kinds, and the difference between them is the useful part:

- **One identifier is named, and the name does not hash to it.** obSCEne's mined table
  attributes it to a real function in `libSceAgc`, from five independent sources - and hashing
  that name produces a *different* identifier, which the same table also carries. This is
  systematic rather than a typo: of 116 published `libSceAgc` identifiers, 108 reproduce from
  their name and 8 do not, and five of those 8 are the *second* identifier on a name whose
  first one reproduces. Ruled out by measurement: name decorations (`_`, trailing digits,
  `Internal`, `Impl`, `Ex`, library-qualified forms) and the empty suffix. So the attribution
  is external evidence, not a derivation, and **a name search can never find it** - which is
  precisely why the asking project's search failed.

- **One identifier is named by nobody**, in either byte order, in 166,971 mined names or in
  1,130,757 identifiers observed without one. Worth stating as a bound rather than a shrug: the
  same corpus publishes `Func_<HEX>` placeholders for identifiers it has seen but cannot name,
  and this one is not among those either, so no public table has *observed* it.

- **One identifier was never a name at all.** It is the hash of the literal string
  `$fYZQG4CU71c` - a symbol in a consumer's own build whose leading `$` is that project's sigil
  for "this name *is* the identifier". Hashed with the sigil attached, it produces a value that
  belongs to no library and can never be named, which is the whole of the reported symptom.

That third one is the reason to write this down. **A hash nothing can name is evidence about a
toolchain, not only about a vocabulary**, and the first question to ask of an unnameable
identifier is whether something hashed a string that was already an identifier. The forward
direction settles it in one run: hash the candidate string and see whether the mystery value
comes back.

The module this repository builds gets it right - `selfish imports` on the consumer's module
leg shows `fYZQG4CU71c` against `libSceAgc`, so `mkmodule` honours the sigil. The plain payload
leg has no vendor tables at all (`0 vendor import entries, 0 DT_NEEDED`), so there the symbol
keeps its literal name and anything hashing that name gets the wrong number. The answer to
"which library ordinal does the entry carry" is therefore neither *valid* nor *invalid*: there
is no vendor import table in that file to carry one.
