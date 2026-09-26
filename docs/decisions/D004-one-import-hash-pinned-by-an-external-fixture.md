# D004 - One import hash, pinned by an external fixture

**Status:** decided
**Date:** 2026-09-26

The import hash has one implementation, `selfish-nid`, used by the probe and the emulator
alike. `tests/known-pairs.txt` pins it with name and encoding pairs taken from the resolution
logs of independent open-source implementations, checked in both directions with the count
asserted.

**Why:** a probe sharing the loader's hash cannot detect a wrong hash by agreement, which is the
argument for two implementations. A fixture produced elsewhere answers it better: it constrains
the suffix, byte order, alphabet and packing against implementations nobody here wrote.

**Rejected:**
- Two independent implementations: they can agree with each other and both be wrong.
- One implementation with no external fixture: nothing outside the repository checks it.
