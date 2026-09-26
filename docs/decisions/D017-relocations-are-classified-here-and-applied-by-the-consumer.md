# D017 - Relocations are classified here and applied by the consumer

**Status:** decided
**Date:** 2026-09-26

`selfish-elf::reloc` reads both relocation tables, splits `info` into type and symbol index,
and says which types need a symbol or thread-local storage. An unknown type has no name.
Applying a relocation stays with the consumer.

**Why:** which types need what is a fact about the format. What to do when a symbol does not
resolve is policy, and the consumers differ: an emulator tallies what it skipped, a builder
fails.

**Rejected:**
- Applying relocations here: forces one consumer's failure policy on the others.
- A generated label for an unknown type: an unhandled type stops being noticed.
