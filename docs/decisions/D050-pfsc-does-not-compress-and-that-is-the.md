# D050 - `PFSC` does not compress, and that is the format rather than a shortcut


The reader implements zlib, so a writer looked like it needed a compressor. It does not. The
block map is a list of absolute offsets, and blocks stored end to end at a fixed stride are a
valid map - a reader hands back any block that is already the full block size. `LibOrbisPkg`'s
writer says so in its own class comment: it writes the header and "doesn't actually do
compression or anything interesting".

Recorded because it was on the blocker list under the wrong description. Compression was never
required; it is an option the format permits and nothing here needs.

