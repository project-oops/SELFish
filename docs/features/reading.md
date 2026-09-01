# Reading a file

Point `selfish` at something and it says what is there. Every reading command is
non-destructive and takes a path.

## What kind of thing is this?

Start with the container, because most files that matter are one:

```bash
selfish container eboot.bin
```

It describes the container, and the executable inside it. If what you have is a bare
executable rather than a wrapped one, `elf` reads it directly:

```bash
selfish elf module.prx
```

## What does it need from the system?

```bash
selfish imports module.prx
```

Undefined symbols in a vendor module are **named by a hash**, not by a string, so this
resolves each one back to a library and a module name where it can. `--all` keeps the ones
it could not resolve, which is the honest view when you are trying to find out what is
missing rather than what is known.

The hash is computable on its own, which is how a name is confirmed rather than guessed:

```bash
selfish nid sceKernelAllocateDirectMemory
```

That takes any number of names and prints the hash for each. A name whose hash matches an
unresolved import is the name - the function is one-way, so a match is proof rather than
evidence.

## What is inside it?

```bash
selfish sections module.prx     # sections and the link-time symbol table
selfish reloc module.prx        # relocation tables, censused by type
```

`sections` takes any number of symbol names after the file and reports where each is
defined, which is the quick way to answer "did this actually get linked in".

`reloc` counts rather than lists. A relocation census is how you tell a module that will
load from one that will load *and then* fail somewhere unrelated: an unexpected type in the
census is a loader requirement nobody has implemented yet.

## What does a title say about itself?

```bash
selfish title package.pkg
```

Takes a package, a `PARAM.SFO`, or a `param.json`, and works out which it was handed. The
metadata a title carries about itself is the same in all three, wrapped differently.

`--round-trip` writes the metadata back out and checks it matches byte for byte. That is a
test of this tool rather than of the file - a parser that cannot reproduce its input has
misunderstood some part of it, and the mismatch says which.

## What is in a package?

```bash
selfish pkg package.pkg         # list the entries
selfish pkg package.pkg --all   # including the ones whose meaning is not established
selfish extract package.pkg --out ./unpacked
```

`--all` matters. A package holds entries this project cannot yet name, and the default view
hides them so the listing is legible. When you are working out what an unknown entry *is*,
the default is the wrong view.

## Where the answers come from

Every structure these commands read is derived from a citable public source and recorded in
`data/` with a header naming exactly where it came from. A real file is used to **confirm or
refute** a structure, never to derive one - see
[conventions §1](https://github.com/project-oops/OOPS/blob/main/docs/CONVENTIONS.md#1-provenance-is-a-hard-boundary)
and this project's own principle 2. Where a field's meaning is unknown it is named `unknown`
and left alone, so an absent answer is visible rather than invented.
