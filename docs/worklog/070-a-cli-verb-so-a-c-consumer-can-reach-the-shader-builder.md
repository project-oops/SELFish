# 2026-09-11 - A CLI verb, so a C consumer can reach the shader builder


`selfish-shader` (worklog 069) is a Rust crate. Its one consumer, obSCEne's dispatch probe, is
freestanding C that drives the `selfish` binary from a shell - it cannot link a Rust crate. So the
crate, as shipped, was unreachable by the project that needs it, and obSCEne could not retire its
disassembled container without a build-time way to generate a clean one.

`selfish shader` is that way. It is a thin wrapper over `Container::compute().build()`:

```
selfish shader (--code <file> | --shader-size N) [--target 0xN] [--sh-reg OFF=VAL]... -o <file>
```

- `--code` records the bytecode's size (the bytes are not embedded - a console fills the code
  pointer), or `--shader-size` gives the size directly.
- `--target` defaults to `0x0e`, RDNA2.
- `--sh-reg OFFSET=VALUE`, repeatable, are the shader's SH registers - the caller's, read from its
  own bytecode. The verb lays them out; it does not interpret them.

It draws the same line the crate does. The container *format* is the tool's; the register
*contents* are the shader's. So the verb warns - it does not refuse - when fewer than two SH
registers are given: a console needs at least the program-address pair (`0x20c`/`0x20d`) it patches
from the code pointer, but which registers a shader carries is the shader's business, and this
crate does not own that rule (D103).

Nothing new about the format; this is only the surface that lets a non-Rust consumer reach it, so
obSCEne's dedup (its inbox 9f4c) has a clean generator to move to. Compute only, because that is the
stage with a consumer - the other stages stay unbuilt until something needs them.
