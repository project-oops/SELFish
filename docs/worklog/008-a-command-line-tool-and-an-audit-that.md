# A command-line tool, and an audit that found the real gap


`selfish-cli` wraps everything that works: `nid`, `elf`, `container`, `wrap`, `pkg`, `extract`.
A thin shim by design - no subcommand holds logic, because a behaviour a command needs is one
the other two consumers need too.

It closes the loop on real material. `extract` pulls 37 files out of a real package, and
`container` reads the `eboot.bin` that comes out - a genuine vendor-toolchain artefact, parsed
by a reader that had only ever seen its own writer's output.

One thing it defaults deliberately: `wrap --generation` is **4**, not 5. That is the
measurement from D010 rather than a habit, and the help text says so.

### The audit, prompted by being asked

Asked whether anything was missing, the honest answer was yes, and it is the largest thing in
the project:

**The vendor dynamic table is 2,200 lines written twice** - 754 reading it in an emulator,
1,446 writing it in a probe. `PT_SCE_DYNLIBDATA`, the `DT_SCE_*` tags, the string and symbol
tables, the relocations. It is what a loader actually reads; everything else about an
executable is preamble.

It is bigger than everything built here today, and it is the file that changed signature twice
this afternoon while callers were updated behind it. That churn is the argument for the shared
crate, demonstrated live.

Six other gaps are now in `docs/BACKLOG.md` with what blocks each - including three that block
on nothing and one that blocks on finding a source rather than on effort. A gap nobody has
written down is one somebody rediscovers by needing it.

