# D101 - Imports are bound global

**Status:** decided
**Date:** 2026-09-26

`dynlib` sets every resolved import to `FUNCTION` and `STB_GLOBAL`, and refuses an unclaimed
one, so a module built here cannot carry a weak undefined import.

**Why:** the loader binds weak imports only from libraries already resident, leaving the others
null (`obscene#D248`). A probe declares every import weak so an absent one links, so preserving
source binding would produce a module whose real imports do not bind. A weak-symbol absence
check therefore cannot survive packaging; it has to use another mechanism.

**Rejected:**
- Preserving `STB_WEAK` from the source: the module's real imports stay unbound.
