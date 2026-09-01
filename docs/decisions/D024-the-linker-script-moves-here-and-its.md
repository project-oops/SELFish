# D024 - The linker script moves here, and its constants are tested against the crate's


`link/module.ld` was obSCEne's and it is format knowledge by any reading: it declares the
vendor segment types, the console's allocation granularity, and the two-loadable-segment
layout. CLAUDE.md already lists "segment layout rules and linker scripts" as belonging here.

What made it worth doing carefully is that **a linker script is the one artefact in this
repository no compiler checks**. A segment type that drifts from `selfish-elf::segment`
produces a module whose headers are wrong and whose build succeeds. So `layout.rs` compiles
the script in with `include_str!` and asserts the constants match, and an integration test
runs a real `ld.lld` over it and reads the result back through this crate's own parser:

```
LOAD           R E     <- two, not three
LOAD           RW
DYNAMIC
INTERP
LOOS+0x1000001         <- PT_SCE_PROCPARAM
LOOS+0x1000000         <- PT_SCE_DYNLIBDATA
```

The link test **skips** rather than fails when `clang` and `ld.lld` are absent. They are not
build dependencies, and a test that fails on a machine without them teaches people to ignore
failures, which costs more than the coverage is worth.

Two references to obSCEne's own tooling were generalised to "the builder" on the way in. The
division of labour those comments describe - a script places, a builder generates - is real
and is the point; which program does it is a consumer's business.

