# D035 - A derived row ships with the command that re-derives it

**Status:** decided
**Date:** 2026-09-26

A format fact established by derivation from material, rather than cited from a source, is
marked `DERIVED` in `data/`, and `selfish derive <package>...` re-checks it against packages the
reader supplies. A claim survives only if every sample it can be tested on agrees.

**Why:** a cited row can be checked by opening its source; a derived row is only as good as its
samples and method, and one nobody can re-run is indistinguishable from memory.

**Rejected:**
- Derived rows without a command: unverifiable by anyone else.
- Majority agreement: a format true of two packages in three is not a format.
