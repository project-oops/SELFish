# D002 - The generation is a type with no default

**Status:** decided
**Date:** 2026-09-26

`Generation` has no `Default`, and every builder takes the target generation explicitly. The
container magic is a `[u8; 4]`, not an integer.

**Why:** the two generations' containers differ only in the magic, so a wrong generation is
structurally perfect and refused by a loader as "not a container". A default hides that
choice. Held as an integer the magic looks right and serialises in the wrong byte order.

**Rejected:**
- A default generation: the caller never states the one thing that differs.
- The magic as a `u32` constant: correct in source, reversed on disk.
