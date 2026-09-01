# Taking it as a library

The command-line tool is a thin layer. Everything it does is in crates you can depend on
directly, and the split between them is deliberate: **it is what keeps cryptography out of
a loader.**

## The spine

Each crate depends only on the ones before it.

| crate | what it holds |
|---|---|
| `selfish-abi` | the generation split, and nothing else. Depends on nothing. |
| `selfish-nid` | the import hash, both directions |
| `selfish-elf` | the executable format as the platform spells it |
| `selfish-container` | the signed-executable container |
| `selfish-pfs` | the filesystem carried inside a package |
| `selfish-pkg` | the package itself |

`selfish-title` sits off to one side. It depends on nothing here and holds what a title says
about *itself* - `PARAM.SFO` and `param.json`. `selfish-pkg` uses it because a package
carries a `PARAM.SFO`; nothing else does.

**An emulator reading a bare executable takes `elf` and stops.** It never links a
cryptography dependency it has no use for, because the crate that needs one is further
along the spine and it never reaches it.

## The generation is in the type system

Two hardware generations share one container and differ in four bytes. A builder that does
not say which one it is targeting **must not compile**.

That is not tidiness. The one time this was a runtime parameter with a default, the default
was wrong and the file was rejected by the machine it was built for. A type error at the
call site costs a minute; a rejected package costs an install cycle and tells you nothing.

## Read and write live together

A format library that can only parse is half a library, and the half that writes is where
the errors are. Where both exist for one structure, the round trip is a test: parse what was
written, write what was parsed, fail if they differ.

This is why `selfish title --round-trip` exists as a command and not only as a unit test -
the same property is worth checking against a file you were handed, not just against
fixtures.

## What you are not promised

None of these crates gets an API shaped around one caller's convenience. A format library
that grows a method because a consumer found it handy ends up encoding that consumer's
assumptions, and the next consumer inherits them without knowing. If you need something
shaped differently, wrap it.

The data is the source of truth, not the code: `data/` holds one row per field with a
provenance header naming every source, and the crates read those tables rather than carrying
their own copy of the numbers.

## Standalone works here, deliberately

SELFish has no dependency outside itself. A clone of this repository alone builds and tests,
which is not true of every project in the collection - obSCEne needs its siblings by
relative path. A format library that could only be built as part of a set would be a poor
thing to ask anyone to depend on.
