<p align="center">
  <img src="assets/logo.png" alt="SELFish" width="200">
</p>

# SELFish

Rust libraries and a command-line tool that read and write the file formats
Prospero-generation and Orbis-generation hardware loads: ELF as the platform spells it, the
signed-executable container, packages and the filesystem inside them, title metadata, the
shader container and the import hash. The `selfish` tool turns a compiler's ELF into a stamped
module, an `eboot.bin`, a title directory or an installable package, and describes any of
those it is given.

Every format is taken from citable public sources, recorded in `data/` with its provenance, and
checked against real files without deriving anything from them. Containers declare themselves
fake and nothing here claims to be the vendor.

Site: [project-oops.github.io/SELFish](https://project-oops.github.io/SELFish/)

## Crates

| Crate | Purpose |
|---|---|
| `selfish-bytes` | bounds-checked integer reads and writes at an offset |
| `selfish-abi` | the generation split |
| `selfish-nid` | the import hash, both directions |
| `selfish-elf` | ELF as the platform spells it: dynamic tables, symbols, relocations, the module writer, linker script layout |
| `selfish-container` | the signed-executable container, both directions, and the audit against the format table |
| `selfish-title` | `PARAM.SFO` and `param.json` |
| `selfish-pfs` | the package filesystem: reading, and writing the inner, `PFSC` and outer layers |
| `selfish-pkg` | packages: reading, the key chain, licences, and the builder |
| `selfish-shader` | the AGC shader container |
| `selfish-cli` | `selfish`, the command line |

Each library depends only on those before it, so a loader that reads a bare executable takes
`selfish-elf` and compiles no cipher. orbistoun, obSCEne and prosperous take these crates by
path.

## Building

A Rust toolchain is the only requirement for the libraries. `selfish-cli` also takes
`oops-build` from oops-libs, which must be checked out beside this repository; the
[OOPS](https://github.com/project-oops/OOPS) collection does that:

```bash
./bin/oops bootstrap selfish    # from the collection root
```

`bin/selfish` is the dev command, and CI runs the same one:

| Verb | Does |
|---|---|
| `check` | the gate (the default): `cargo fmt --check`, clippy at `-D warnings`, the tests, a doc build |
| `build` | release build of the workspace |
| `test` | `cargo test --workspace` |
| `lint` | clippy at `-D warnings` |
| `fmt` | format in place |
| `doc` | build the API docs |
| `clean` | remove build output |
| `provenance` | every table in `data/` names its sources, and no executable or package is tracked |
| `links` | the tests that link through `link/*.ld` ran rather than skipping for a missing `ld.lld` |

The linking tests need `clang` and `ld.lld` and skip without them; `links` fails if they
skipped.

## Using it

Building is one invocation:

```bash
selfish --input app.elf --target prospero --format title --title-id GLCB00001 --output build/title
```

`--format` is `elf`, `prx`, `eboot`, `title` or `pkg`; `--target` is `orbis`, `neo`,
`prospero` or `trinity` and has no default. Everything else is a subcommand that describes a
file: `elf`, `imports`, `sections`, `reloc`, `container`, `audit`, `title`, `pkg`, `extract`,
`derive` and `nid`, plus `image`, `pack` and `shader`, which build a package's parts and a
shader container. `selfish --help` lists them.

[docs/USER_GUIDE.md](docs/USER_GUIDE.md) covers each format, the options and the reading
commands; [docs/GLOSSARY.md](docs/GLOSSARY.md) the vocabulary; [docs/DECISIONS.md](docs/DECISIONS.md)
the decisions in force.

## Licence

MIT or Apache-2.0, at your option. Sources consulted are in
[ACKNOWLEDGEMENTS.md](ACKNOWLEDGEMENTS.md).
