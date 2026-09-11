# 2026-09-11 - Pixel and vertex stages, the hardware that confirmed the header, and the constants I would not ship


Two things arrived together: obSCEne resolved the differential-probe ask (9f4c) with per-field
hardware verdicts, and filed a new ask (4d1f) to extend `selfish-shader` past compute. Doing the
second honestly meant not doing all of what it asked.

## The probe confirmed the header, field by field

`166-agc/shader-differential` (sweep 20260911-002219) built a clean container and perturbed one
field at a time on RDNA2 silicon. It is the hardware oracle the emulator-derived layout was waiting
for, and it landed in the table's header:

- **Checked** (the console rejects a bad value): magic (`INVALID_MAGIC`), version (must be `0x18`,
  `UNSUPPORTED_VERSION`), type-vs-registers and `num_sh_registers=0` (`INVALID_STAGE_REGISTERS`),
  and the `user_data`/`sh_registers` pointers (SIGSEGV - the driver dereferences them).
- **Tolerated at create**: `cx_registers`, `specials`, `header_size`, `shader_size`, `target`, an
  invalid type when its registers still match, and a non-256-aligned code pointer (the command
  processor enforces alignment at dispatch, not creation).

"Tolerated at create" is not "may be wrong" - `shader_size`/`target`/alignment are enforced later,
so they are still written correctly. The probe measures what `sceAgcCreateShader` guards.

## Pixel and vertex: clean, because only one byte changes

The container format is stage-agnostic - only the `type` byte at `0x5a` differs. So
`Container::pixel` (1) and `Container::vertex` (2) are the compute constructor with a different
value, and a test pins that a pixel or vertex container differs from a compute one *at that one
byte and nowhere else*. Both values are sourced: craziiEmu maps `1=PS`, `2=ES` (the export-shader
hardware stage a vertex shader runs in), and obSCEne's `166-agc/primitive-draw` draws with them.

## The constants I would not ship, and why

4d1f asked for six stage constants: `COMPUTE=0`, `PIXEL=1`, `VERTEX=2`, `GEOMETRY=3`, `EXPORT=6`,
`HULL=7`. Three of those do not survive the only citable source for the header's `type` byte,
craziiEmu's register switch (`0=compute, 1=PS, 2/6=ES, 4=GS, 7=LS`):

- `GEOMETRY=3` **conflicts** - craziiEmu has geometry at `4` (GS) and no `3` at all.
- `HULL=7` **mislabels** - `7` is LS (the vertex stage feeding a hull shader), not the hull shader.
- `EXPORT=6` is just craziiEmu's `ES` renamed - consistent, but redundant with `2`.

None of `3/6/7` appear in the 9f4c evidence either. So I shipped constants only for the three
values a source establishes (`0/1/2`), and for anything else added `Container::for_stage(type, ...)`
- a general primitive that lays out a container around a `type` value the *caller* has confirmed,
without this crate asserting a meaning it cannot cite. obSCEne can build a geometry container today
by passing the value its own hardware confirms; it just will not get a `STAGE_GEOMETRY = 3` from
here until `3` is shown to be the value. The conflict is filed back to obSCEne (its inbox).

This is the same line the whole crate draws, one level up: the *format* is ours to know, and a
*stage value* is a fact that needs a source like any other. A named constant is a claim; `for_stage`
is the honest way to build what is real without making one. (D103)

## The CLI came along

`selfish shader --stage compute|pixel|vertex` (or a raw type value) exposes the new constructors to
the C consumer, since obSCEne reaches the builder only through the binary. Same warning as before
when the SH register table is too thin, now stage-aware - compute's program pair is `0x20c/0x20d`,
other stages' pairs are their own.
