# CLAUDE.md

Rules SELFish adds to the shared ones. Read [AGENTS.md](../AGENTS.md),
[CONVENTIONS](../docs/CONVENTIONS.md) and [STYLE](../docs/STYLE.md) first; this file does not
restate them.

## Purpose

Rust libraries and a command-line tool that read and write the file formats
Prospero-generation hardware loads: ELF as the platform spells it, the signed-executable
container, packages and the filesystem inside them, title metadata, the shader container, and
the import hash. It is the one place orbistoun, obSCEne and prosperous get these formats from.

## Admission test

A thing belongs here when it holds knowledge about a file format and nothing that knows what a
consumer is for. For a borderline case: would it still make sense if the consuming projects
did not exist?

- **In:** file formats, the import hash, segment layout rules, linker scripts.
- **Out:** the mined identifier corpus (obSCEne's measurement output), and anything that
  executes on the hardware - a `crt`, an allocator, string functions, a homebrew convenience
  layer. Knowledge, not runtime (D003).
- **Borderline:** platform ABI declarations. A move here carries each signature's provenance
  level with it, not just the signature (D003).

## Principles

- **Formats come from nameable sources.** Structure comes from published documentation and
  open-source implementations, each cited by project and commit.
- **Real files are an oracle, never a source.** A package or executable in hand confirms or
  refutes a structure taken from cited sources; it never derives one. This is stricter than
  the shared provenance rule. The `data/` header records which rows real files settled, and
  the material itself is never committed.
- **The generation is in the type system.** A builder that does not state its target
  generation does not compile; there is no default (D002).
- **Read and write live in one crate.** Where both exist for a structure, a round trip is a
  test: parse what was written, write what was parsed, fail on any difference.
- **Nothing is invented.** A field of unknown meaning is named `unknown` and left alone. An
  absent table row is visible; an invented one is not.
- **Fake, and it says so in a field.** Only the public fake-package keysets are used; nothing
  here works on retail material. Containers declare themselves fake in the format's own field
  and their signature areas are zero. A package licence is signed with the published debug RIF
  keyset and declares itself a debug licence in its type field. Never claim to be the vendor
  (D047).

## Consumers

orbistoun, obSCEne and prosperous depend on these crates. None gets an API shaped around its
own convenience; a consumer that needs a different shape wraps.

Conventions of a probe's symbol tables stay in obSCEne: the manifest saying which library
resolves which name, and the `$` sigil marking a symbol whose name is the identifier.

## Where things live

- `data/` - the format tables, one row per field, each with a provenance header. They are the
  source of truth; code reads them and carries no copy.
- `crates/` - the dependency spine, in build order; each depends only on those before it, which
  keeps cryptography out of a loader:
  - `selfish-bytes` - bounds-checked integer reads and writes at an offset; depends on nothing.
    Every crate reads and writes fields through it.
  - `selfish-abi` - the generation split; depends on nothing.
  - `selfish-nid` - the import hash. It precedes `elf` because a vendor module's undefined
    symbols are named by the hash (D015).
  - `selfish-elf` - ELF as the platform spells it. An emulator reading a bare executable stops
    here.
  - `selfish-container` - the signed-executable container.
  - `selfish-pfs` - the filesystem inside a package.
  - `selfish-pkg` - packages.
- Off the spine, depending on nothing in this repository but `selfish-bytes`:
  - `selfish-title` - what a title says about itself: `PARAM.SFO` and `param.json`. `pkg` uses
    it; nothing else does.
  - `selfish-shader` - the shader container, from `data/agc-shader-format.tsv`. It owns the
    header and sub-table placement; register contents come from the caller (D103).
- `selfish-cli/` - the command-line tool.
- `link/` - the linker scripts.
- The crate list above stays complete; a crate missing from it gets written a second time
  (D062). The members are in `Cargo.toml`.

## Gate

`./bin/selfish check`
