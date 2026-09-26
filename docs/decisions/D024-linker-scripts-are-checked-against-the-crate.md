# D024 - Linker scripts are checked against the crate

**Status:** decided
**Date:** 2026-09-26

The linker scripts live in `link/`. `selfish-elf::layout` embeds them and asserts their segment
types and constants match the crate's, and integration tests link a real object through them
with `ld.lld` and read the result back. The link tests skip without `clang` and `ld.lld`;
`./bin/selfish links` and CI fail if they skipped.

**Why:** a linker script is the one artefact no compiler checks. A segment type that drifts
from `selfish-elf` produces a module whose build succeeds and whose headers are wrong.

**Rejected:**
- Scripts in each consumer: the same constants in several unchecked copies.
- Failing the link tests without a toolchain: they are not build dependencies, and a test that
  fails on an ordinary machine teaches people to ignore failures.
