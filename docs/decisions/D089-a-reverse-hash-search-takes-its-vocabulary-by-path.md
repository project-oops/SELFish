# D089 - A reverse hash search takes its vocabulary by path

**Status:** decided
**Date:** 2026-09-26

Finding a name for an import hash is `examples/name_nid`, which hashes every word of a
vocabulary file the caller passes, searches a raw value in both byte orders, and prints the
encoded form of each reading.

**Why:** the hash is one-way, so the vocabulary is the whole answer. The mined identifier corpus
belongs to obSCEne; a tool carrying its own word list would be a second copy of it. Projects in
the collection print the same identifier in both byte orders.

**Rejected:**
- A built-in vocabulary: a drifting copy of another project's corpus.
- Searching one byte order: silently misses half the callers.
