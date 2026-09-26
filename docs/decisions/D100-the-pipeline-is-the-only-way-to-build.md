# D100 - The pipeline is the only way to build

**Status:** decided
**Date:** 2026-09-26

Building is one invocation: `--input`, `--target`, `--format` (`elf`, `prx`, `eboot`, `title`,
`pkg`) and `--output`, with no stamping or wrapping subcommands. An option reaches only the
formats it affects and is refused elsewhere: `--privilege` and `--sdk` for the formats that build
a container, the title metadata options for `title` and `pkg`. A format writes exactly
`--output`; the pipeline takes no directory of extra files.

**Why:** one path means one set of defaults and checks. An option that silently does nothing is
how somebody believes they set it. A caller-prepared directory would break the input-to-output
contract that lets builds run in parallel.

**Rejected:**
- Separate `stamp` and `wrap` verbs: a second route with its own defaults.
- Ignoring inapplicable options: silent no-ops.
- A `--root` directory merged into a title: extra files are copied beside the output instead.
