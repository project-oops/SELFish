# D028 - The builder states the object type

**Status:** decided
**Date:** 2026-09-26

`identity::stamp` writes `EI_OSABI`, `EI_ABIVERSION` and `e_type`, with `e_type` and the
generation as parameters. It reports only the fields it changed, and refuses an `e_type` that is
neither a linker's `ET_DYN` nor a platform type.

**Why:** no linker sets these fields, a loader checks them first, and an executable and a shared
library are both legitimate outputs that only the builder can tell apart. Overwriting an
unrecognised type would assert something untrue about a file this code does not understand.

**Rejected:**
- A fixed `e_type`: a shared library would inherit the executable's type.
- Rewriting any `e_type`: a relocatable object would be stamped as an executable.
