# D099 - Package entries derivable here are computed

**Status:** decided
**Date:** 2026-09-26

The package builder computes entry `0x200` (the names of the entries present) and entry
`0x1001` (the single-chunk `playgo-chunk.dat`, from `data/pkg-format.tsv`, the package size and
the inner filesystem size read from the image) when the caller does not supply them. A supplied
one is used as given.

**Why:** both are fixed structures whose variable parts the builder already holds, so requiring
them asks the caller to hand back facts the builder has. Rebuilding an existing package byte for
byte still needs its own entries.

**Rejected:**
- Requiring them from the caller: callers without them hardcode or guess.
- Ignoring a supplied entry: prevents reproducing existing material.
