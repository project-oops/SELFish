# D091 - A writer refuses where a reader may guess

**Status:** decided
**Date:** 2026-09-26

Code whose output is written back into a file refuses input it cannot represent exactly:
`dynlib::string_at` returns `BuildError::SymbolName` for an out-of-range offset, a missing
terminator or invalid UTF-8. A reader may be lenient where leniency can only produce a wrong
line of output.

**Why:** a reader's guess is a wrong line on a terminal; a writer's guess is a wrong file. A
lossy builder renamed symbols and, given the wrong string table, wrote undefined symbols back as
nameless locals, skipping the unclaimed-import check.

**Rejected:**
- One permissiveness for both sides: either the reader refuses real files or the writer
  invents names.
