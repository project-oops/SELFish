# SELFish documentation

Rust libraries and a command-line tool for the platform's own file formats - read and
written in one place. The executable format as the platform spells it, the signed-executable
container, packages and the filesystem inside them, and the import hash. Nothing else.

New here? The [root README](../README.md) has the pitch. Then:


## The words

- [GLOSSARY.md](GLOSSARY.md) - NID, SELF and fSELF, PFS, packages, the generation split and
  what the four magic bytes actually distinguish. This is the vendor half of the collection's
  vocabulary; the standard-ELF half is the collection's glossary.

## Guide

- **[reading.md](features/reading.md)** - point it at a file and find out what is there.
  Containers, executables, imports, relocations, packages, and what a title says about
  itself.
- **[writing.md](features/writing.md)** - the four steps between a compiler and something
  the hardware will install, and how to run each one on its own.
- **[library.md](features/library.md)** - taking the crates directly, the dependency spine,
  and why the split is what keeps cryptography out of a loader.
- **[BUILDING.md](BUILDING.md)** - `bin/selfish`, what `check` runs, why it is a script rather
  than a command somebody types, and the two tests that skip silently without `lld`.

## The command

The binary is `selfish`. Reading commands take a path and print; writing commands are the
four format steps.

```
nid        hash a symbol name the way a loader does
elf        describe an executable
imports    what it imports, resolved to library and module names
sections   sections and the link-time symbol table
reloc      relocation tables, censused by type
container  describe a container, or the executable inside it
title      what a title says about itself
pkg        list what is inside a package
extract    extract a package's files
derive     re-derive what a package's entries mean, from packages you supply

stamp      the platform identity a loader checks first
wrap       wrap an executable in a container
image      build the filesystem image from a directory
pack       assemble the package
```

`selfish <command> --help` is the authority on flags - it is generated from the code, so it
cannot drift from it the way this list can.

## Why this repository exists

Three projects need these formats. orbistoun reads them to load a title, obSCEne writes them
to produce one, Prosperous inspects them over a wire. Before this repository the knowledge
lived in whichever project happened to need it first, and the cost was not hypothetical: a
container builder shipped emitting the *previous* generation's magic, while the current
value sat recorded in another project's decision log one directory away.

The full argument, and the admission test for what belongs here, is in
[CLAUDE.md](../CLAUDE.md).

## The rules this is held to

Formats come from sources that can be named. **A real file is an oracle, never a source** -
used to confirm or refute a structure taken from cited material, never to derive one.
Nothing is invented: where a field's meaning is unknown it is named `unknown` and left
alone, because an absent row is visible and a wrong one is not.

Shared rules - provenance, naming, decision logs, worklogs, gates - are in
[the OOPS conventions](https://github.com/project-oops/OOPS/blob/main/docs/CONVENTIONS.md)
and not restated here.

## Project memory

- [DECISIONS.md](DECISIONS.md) - a generated index over `decisions/`, one file per
  entry. Every non-obvious choice and the reasoning, including the
  ones reversed on evidence
- [WORKLOG.md](WORKLOG.md) - a generated index over `worklog/`, in document order.
  What happened, surprises especially
- [BACKLOG.md](BACKLOG.md) - a generated index over `backlog/`, with a status column.
  What is missing and what blocks it

## Adding to a log

The long-running documents are **directories with a generated index**. Add a file under
`decisions/`, `backlog/` or `worklog/`, then regenerate the table:

```bash
tools/split-decisions.sh --index selfish
tools/split-doc.sh --index selfish BACKLOG 2 backlog
```

Do not edit the index by hand - it is overwritten. The split exists because two sessions
appending to one file collide, which is where the duplicate numbers and out-of-order entries
came from, and because a log past half a megabyte stops rendering on GitHub entirely.
