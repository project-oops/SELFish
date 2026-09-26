# D015 - `selfish-elf` depends on `selfish-nid`

**Status:** decided
**Date:** 2026-09-26

`selfish-nid` precedes `selfish-elf` on the dependency spine, and the ELF reader decodes a
vendor module's `<hash>#<library>#<module>` symbol names itself.

**Why:** reading the symbol table and decoding the hash are one operation for any consumer that
resolves imports, which is every consumer that reads the table. The cost is `sha1` on the
loader path, which such a consumer needs anyway.

**Rejected:**
- Leaving the decode to each consumer: three copies of the same join, each able to be wrong
  silently.
