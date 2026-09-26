# D050 - `PFSC` is written uncompressed

**Status:** decided
**Date:** 2026-09-26

`selfish_pfs::pfsc::wrap` stores every block at the full block size, end to end, with a block
map of absolute offsets. The reader still decompresses zlib blocks.

**Why:** the format permits uncompressed blocks - a reader returns a full-size block as is - and
`LibOrbisPkg`'s writer does the same. Nothing here needs the smaller image.

**Rejected:**
- Compressing with zlib: a dependency and a code path for an option the format does not require.
