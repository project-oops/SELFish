<p align="center">
  <img src="assets/logo.png" alt="SELFish" width="200">
</p>

# selfish

**Rust libraries and a command-line tool for the platform's own file formats** - plus the
linker script and target runtime that go with them.

Site: **[project-oops.github.io/SELFish](https://project-oops.github.io/SELFish/)**

**Reading:** point it at a file and it says what is there - what an executable imports, how its
relocations break down, what is inside a package, what a title says about itself.

**Writing:** hand it a freshly compiled binary and it produces something the hardware will install -
the platform identity no linker sets, the signed-executable container, the filesystem image, and
the package around it.

Between a compiler and the hardware there are four format steps. This is all four, and the readers
that let you check each one.

Named for what it mostly produces: a container that resembles a signed executable without
being one. A **SELF-ish** file, declaring itself fake in the field the format provides for
exactly that.

## What is here

A dependency spine, in build order. Each crate depends only on those above it, and the split
is not organisational - it is what keeps cryptography out of a loader.

| crate | what it holds |
|---|---|
| `selfish-abi` | the generation split. Depends on nothing |
| `selfish-nid` | the import hash, and the library and module ids a symbol name encodes |
| `selfish-elf` | the executable format: headers and the identity a loader checks, segments, sections, the vendor dynamic table read **and** written, relocations, and the segment layout rules |
| `selfish-container` | the signed-executable wrapper, both directions |
| `selfish-title` | what a title says about itself: `PARAM.SFO` and `param.json` |
| `selfish-pfs` | the filesystem inside a package |
| `selfish-pkg` | packages. The only crate that pulls in RSA, AES and zlib |
| `selfish-cli` | one binary over the above, so the libraries can be pointed at real files |
| `runtime/` | the shared target runtime and SDK: `crt0.c`, `escalation.c`, `include/escalation.h`, `installer.c` |
| `data/` | the format tables - one row per field, each with a header naming where it came from |
| `link/module.ld` | the linker script that lays out a module |

## Try it

```
selfish elf      <file>   describe an executable, unwrapping a container if there is one
selfish imports  <file>   what it imports, resolved to library and module names
selfish reloc    <file>   census its relocation tables, and join the linkage table to the imports
selfish sections <file>   an object's sections and its link-time symbol table
selfish stamp    <file> --generation 4|5 [--library]
                          the header identity a loader checks first, which no linker sets
selfish title    <file>   what a title says about itself, from a package, a .sfo or a .json
selfish pkg      <file>   what is inside a package
selfish derive   <file>...  re-derive what a package's entries mean, from packages you supply
selfish image    --root <dir> -o <file> --content-id ID
                          build the filesystem image a package carries, from a directory
selfish pack     (--image <file> | --dir <dir>) -o <file> --content-id ID
                 [--title-id ID] [--title NAME] [--passcode P] [--entry ID=FILE]...
                          assemble a package; refuses to invent what it cannot account for.
                          --dir does the whole chain from a directory of files in one step
selfish extract  <file> <dir>
selfish wrap     <file> --generation 4|5
selfish nid      <name>...
```

## What is not here

**A consuming project's data.** The import hash is a format and lives here. A corpus of mined
identifiers is a measurement product and stays with the project that mines it.

**Anything retail.** The public fake-package keyset only, and nothing here should be made to
work on signed material.

## Provenance

Every structure comes from published documentation or open-source implementations, cited by
project and commit in **[ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md)**. No format here was worked
out by reading a vendor binary, and no vendor signature is forged.

Real files are an **oracle, never a source** - used to confirm or refute a structure taken from
cited sources, never to derive one - and none of that material is committed here.

The rules in full, including what *is* signed and why that is the opposite of a forgery, are in
**[CLAUDE.md](CLAUDE.md)** and the shared
**[OOPS conventions](https://github.com/project-oops/OOPS/blob/main/docs/CONVENTIONS.md)**.

## Working on it

See [CLAUDE.md](CLAUDE.md) for the constraints, [docs/DECISIONS.md](docs/DECISIONS.md) for why
things are the way they are, and [docs/BACKLOG.md](docs/BACKLOG.md) for what is missing and
what blocks it.

**The recommended way in is [OOPS](https://github.com/project-oops/OOPS)**, which holds all four
side by side and carries one entry point over them:

```bash
./bin/oops check selfish      # also: build, test, fmt, clean
```

That relays to this repository's own entry point rather than reimplementing anything, so the
two cannot disagree - and it is what CI runs, for the same reason.
[docs/BUILDING.md](https://github.com/project-oops/OOPS/blob/main/docs/BUILDING.md) has every verb.

**From inside this repository the entry point is `bin/selfish`**, carrying the same verbs:

```bash
./bin/selfish check   # everything that has to pass. What CI runs.
```

`check` is `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test
--workspace`, and a doc build. `unsafe_code` is forbidden workspace-wide.

It is a script rather than a pipeline because a pipeline swallowed a failure twice - `cargo test
| grep` reports the exit status of `grep`. The script runs each step under `set -e` and filters
the output afterwards.

Two integration tests shell out to `clang` and `ld.lld` to link a module with the script here
and read the result back. They **skip** rather than fail when those are absent - they are not
build dependencies, and a test that fails on a clean machine teaches people to ignore failures.
`./bin/selfish links` runs those tests and fails if they skipped, which is the only way a run
can claim to have checked the linker script.

**The libraries depend on nothing outside this repository, and that is the point of them.**
`selfish-cli` does not: it takes `oops-build` and `oops-log` from `oops-libs` by relative path,
so a clone of only this repository no longer builds the whole workspace. That is a regression
against the property this repository exists to hold, and it is written down rather than
quietly accepted - see **[docs/BUILDING.md](docs/BUILDING.md)**, which is also the full account
of every verb and what CI runs.

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option -
the Rust ecosystem convention.

## Part of OOPS

SELFish is one of four projects aimed at the same platform's operating system. They are developed
together in **[OOPS](https://github.com/project-oops/OOPS)** and released separately.

| | |
|---|---|
| **[Orbistoun](https://github.com/project-oops/Orbistoun)** | the emulator - attempts to reimplement what a title runs on |
| **[obSCEne](https://github.com/project-oops/obSCEne)** | the probe - a guest that interrogates whatever runs it and reports what it found |
| **[Prosperous](https://github.com/project-oops/Prosperous)** | the instrument - remote management for anything that runs Orbis software |

**Developing any of them?** Clone [OOPS](https://github.com/project-oops/OOPS) - it holds all four side by side, arranged so
they build against each other. Cloning this repository alone gets you this project; it is
the right thing for using it and the wrong thing for changing it.

Shared rules - provenance, naming, decision logs, worklogs, gates - live in
[the OOPS conventions](https://github.com/project-oops/OOPS/blob/main/docs/CONVENTIONS.md) and are not restated here.
