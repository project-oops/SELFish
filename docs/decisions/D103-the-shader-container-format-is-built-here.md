# D103 - The shader container format is built here

**Status:** decided
**Date:** 2026-09-26

`selfish-shader` builds the AGC shader container `sceAgcCreateShader` is handed, from
`data/agc-shader-format.tsv`: the header, the sub-table placement and the self-relative pointer
fields. The register contents come from the caller, read from the shader's bytecode by whatever
produced it.

**Why:** the header is cited by five open-source emulators that agree on it, one pinning the
offsets with a `static_assert`, and obSCEne's hardware sweep confirms it. Laying out a container
is format knowledge; finding a shader's registers in its bytecode is what a compiler knows.

**Rejected:**
- A layout from disassembly of the vendor library: not an admissible source, whatever the
  hardware accepts.
- Parsing bytecode here: compiler knowledge, outside the admission test.
