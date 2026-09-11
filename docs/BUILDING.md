# Building SELFish

There is one command and it is `bin/selfish`. Every verb is the same command CI runs.

```bash
./bin/selfish check
```

## What you need

**A Rust toolchain**, and nothing from the hardware - no SDK, no firmware, no keys.

### It depends on nothing outside itself

`bin/selfish` says SELFish depends on nothing outside itself, and its CI comment calls that
"the one property to preserve if anything is ever added here". It holds: a clone of only this
repository builds with a Rust toolchain and nothing else.

It was briefly false. `selfish-cli` took `oops-build` and `oops-log` from `oops-libs` by
relative path for two conveniences - a git-commit stamp on `--version`, and a logging
subscriber - so a clone of only this repository did not build without fetching a sibling first.
Both are gone (D104):

- the commit stamp is now a few lines of git in `selfish-cli/build.rs` (`emit_commit`), reading
  the same `OOPS_COMMIT` a CI workflow can still override;
- the logging was **dead weight** and was removed rather than reimplemented - nothing in these
  crates emits a tracing event, so the subscriber wrapped nothing.

Being the bottom of the spine is what lets the other three take these formats without inheriting
anything, and two conveniences in the CLI were a thin reason to have given it up. The crates
under `crates/` never depended on oops-libs; now `selfish-cli` does not either, and CI needs no
bootstrap step to build this project.

### `clang` and `lld`, for the tests that link

The integration tests that link with this repository's own linker script (`link/module.ld`)
build a real module and read the result back. Without `ld.lld` they **skip** rather than fail.

That skip is why there is a verb for it:

```bash
./bin/selfish links
```

It runs those tests with output shown and **fails if they skipped**, because a skip would let
a run pass having verified none of the segment layout or tag conventions - the most valuable
tests in the suite turning into a silent no-op while the gate stays green.

It is deliberately **not** in `check`. Folding it in would make the local gate slower for
everybody to catch a thing that only goes wrong on a clean machine. CI runs it as a separate
step, and it is a verb rather than inline shell so that a person can run exactly the same
thing.

## The seven shared verbs

So `oops test selfish` and `./bin/selfish test` are one command reached two ways.

| verb | what it does |
|---|---|
| `build` | `cargo build --release --workspace` |
| `test` | `cargo test --workspace` |
| `lint` | clippy, `--all-targets --all-features`, at `-D warnings` |
| `fmt` | `cargo fmt --all` |
| `check` | the gate - see below |
| `clean` | `cargo clean` |
| `doc` | the API docs, with rustdoc warnings as errors |

## SELFish's own two

| verb | what it does |
|---|---|
| `provenance` | every format table names its sources, and no material is committed |
| `links` | the linker tests really ran |

### `provenance`

Two checks, and both are about what this repository is worth rather than about whether it
compiles.

Every `data/*.tsv` must carry a `# read-from:` header naming where its rows came from. A
table whose header stops naming that has quietly become somebody's memory.

And no `.pkg`, `.elf`, `.prx`, `.sprx`, `.bin` or `.self` is tracked. A real file is an
**oracle, never a source** - used to confirm or refute a structure taken from cited material,
never to derive one - and committing one beside a format is that principle failing in the
most direct way available. [`CLAUDE.md`](../CLAUDE.md) principle 2 states the rule; this says
what the gate does about it.

## What `check` runs

1. `cargo fmt --all --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --workspace`
4. `cargo doc --no-deps --workspace`, with broken intra-doc links, invalid HTML tags and bare
   URLs all as errors

`-D warnings` because the lint set here is not decoration. `arithmetic_side_effects`,
`indexing_slicing` and `unwrap_used` are on deliberately: a builder wrong by one byte is
reported by a loader as "not a container" rather than as an offset error, and that is a very
long afternoon.

The doc build is in the gate because documentation is this repository's durable product - the
decision log, the provenance notes, the reasoning attached to every constant. It was not in
the gate once, and an audit found `cargo doc` had been failing on an ambiguous intra-doc link
for some time.

### Why `check` is a script and not a command someone types

It was a typed command for a while, improvised per run, and it swallowed a failure twice:

```sh
cargo test --workspace | grep -oE '[0-9]+ passed' | awk '{s+=$1}...'
cargo clippy ... | grep -E '^(warning|error)'
```

A pipeline's exit status is the **last** command's, so `grep` finding nothing reported success
while the compiler was failing behind it. Once, the test count silently fell from 162 to 116
and the gate still said it was fine.

The rule the current script is written to - obSCEne's `scripts/verify.sh` carries it too - is
that **output filtering never sits between a command and its exit code.** Where filtered
output is wanted, the command runs into a file and the file is filtered afterwards. That is
why `check` writes to a log and tails it on failure rather than piping.

## What CI runs

`.github/workflows/ci.yml`, two jobs:

| job | steps |
|---|---|
| `gate` | install `clang` and `lld`, `oops bootstrap selfish`, `oops check selfish`, then `./bin/selfish links` |
| `provenance` | `oops bootstrap selfish`, then `./bin/selfish provenance` |

Both begin with the standard OOPS preamble - check out the collection, check this repository
out into it, bootstrap. `provenance` needs no toolchain and no sibling, and takes the preamble
anyway: one shape everywhere is what keeps a job from being quietly special.

The provenance step is a block scalar rather than a bare `run:` with a second indented line.
That folds into one plain scalar - `./bin/selfish provenance echo "no material committed"` -
so the verb swallowed the echo as an argument it ignores, and the line never printed.

The clang install is not optional decoration: without it the linker tests skip, and leaving it
out would turn the most valuable tests in the suite into a silent no-op with CI still green.

## The binary

```bash
./bin/selfish build
./target/release/selfish elf <file>
```

`selfish <command> --help` is the authority on flags - it is generated from the code, so it
cannot drift from it the way a list in a document can. [reading.md](features/reading.md) and
[writing.md](features/writing.md) are the guides to what the commands are *for*.

## From the collection

[OOPS](https://github.com/project-oops/OOPS) holds all four side by side:

```bash
./bin/oops check selfish        # also: build, test, fmt, clean
```

That relays to this script rather than reimplementing anything, so the two cannot disagree.
[The collection's BUILDING.md](https://github.com/project-oops/OOPS/blob/main/docs/BUILDING.md)
covers the collection-level verbs.
