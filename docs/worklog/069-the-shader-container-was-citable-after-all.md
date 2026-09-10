# 2026-09-10 - The shader container was citable after all, once I looked past the first two emulators


obSCEne asked SELFish to build the `sceAgcCreateShader` container (7a3b). I ruled on it in D103:
admissible as a format, most of it citable from craziiEmu, but the working-critical fields known
only from a vendor disassembly - so not shippable. I committed and pushed that. Then the operator
asked, in effect, whether I had looked hard enough. I had not.

## The ruling was made on half the evidence

D103 checked craziiEmu and shadPS4 and stopped. There were **four more open-source PS5 emulators in
the same directory** I did not open. Opening them overturned the ruling in minutes:

- **prosper** (`hle_agc.cpp`) has `struct AgcShader` with a `static_assert` pinning every offset,
  and it cites Kyty as its source.
- **KytyPS5** (`shader.h`) has the identical `struct Shader`, plus the sub-table structs.
- **Kyty** (`Shader.h:974`) is the upstream the other two trace to.
- **SharpEMU** validates the same magic and version.

Five readers, one layout. That is stronger provenance than most of this repository's formats have.
And it settled the fields D103 called uncitable: `target` is a `u32` at `0x4c`, and `0x44` is
`shader_size`, not the "64-bit hash" the disassembly draft claimed. **The citable sources caught two
errors in the vendor-RE spec** - which is the entire argument for principle 1, demonstrated rather
than asserted.

This is the recurring fault in its plainest form: I stopped searching when I had *an* answer, not
*the* answer, and ruled. The discriminator was one `ls` of the emulator directory. D103 now carries
the correction at its head, with the full field map, and keeps the original reasoning below as the
record of a ruling made too early.

## What shipped, and the line it draws

`selfish-shader`, a new crate off to the side of the spine like `title` - it depends on nothing.
`data/agc-shader-format.tsv` carries the header layout with all five sources in its provenance
header. `Container::build` lays out the header and the register sub-tables, wiring each pointer
field to the **self-relative offset** a console relocates (craziiEmu's `RelocatePointerField` read
backwards: the reader does `field += fieldAddr`, so the writer sets `field = target - fieldAddr`).
`relocate` does what the console does, so the round-trip test reads the sub-tables straight back.

The admission-test line it draws is worth keeping: **the crate owns the container format and takes
the register contents from the caller.** Which registers a shader programs, and its resource usage,
are read out of the shader's own RDNA2 bytecode - that is a compiler's knowledge, not a format's.
Laying out the container is the format. So `build` places the registers it is handed and does not
interpret them, the same split `pkg` draws between the format it knows and the title identity it is
given.

Six tests: the magic and version five readers validate; the offsets prosper's `static_assert`
pins, plus the two the draft got wrong; the scalar metadata; the self-relative round trip; that an
empty array leaves a null pointer rather than an offset to nothing; and that a sub-table never lands
inside the header it points out of.

## What it does not do, on purpose

It does not parse RDNA2 bytecode to derive the registers - that is the caller's, and inventing it
here would be principle 5. It builds the compute case the request needed and leaves the other stages
to when something needs them. And the container's byte-exactness is confirmed against five readers
and obSCEne's passing sweep as the oracle, not against a committed artifact - no shader binary or
vendor library is in this repository, and none should be.

The disassembly draft that started this (the dropped worklog `069`, `b91f09b`) is gone from history;
this entry takes its number, and its lesson is the opposite of what it recorded: the format did not
need a vendor binary, only a wider look at what was already open-source. (D103)
