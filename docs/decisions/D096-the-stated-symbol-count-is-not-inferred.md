# D096 - The stated symbol count is not inferred

**Status:** decided
**Date:** 2026-09-26

`Info::symbol_count` returns `symtabsz / syment`, and `None` when the module does not state
both. `dynamic::symbols` reads to the end of the segment when `symtabsz` is absent. Both
behaviours stay, and each says so in its documentation.

**Why:** `symbol_count` is a claim about what the module states, and a module that states
nothing has no count. `symbols` is a reader, and stopping at a missing size field would read
nothing where it could read almost everything.

**Rejected:**
- Inferring the count in `symbol_count`: a count that changes when something else moves.
- Refusing in `symbols`: a reader that fails where it could succeed.
