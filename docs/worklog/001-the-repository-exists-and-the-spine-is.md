# The repository exists, and the spine is laid


Six crates in dependency order: `abi`, `elf`, `container`, `nid`, `pfs`, `pkg`. Each depends
only on those before it. The split is not organisational - it is what keeps cryptography out
of a loader, so an emulator reading a bare executable takes `elf` and stops rather than
compiling a cipher it will never call.

`unsafe_code = "forbid"` across the workspace. A format library that needs a raw pointer has
reached for one where a bounds check belonged.

