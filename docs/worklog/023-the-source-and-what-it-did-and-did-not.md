# The source, and what it did and did not settle


Fetched and read LibOrbisPkg - an open-source PS4 packaging tool, and the project this
repository already took its fake keyset from. It named all six unknown entries.

**The four derivations were right.** `0x1`, `0x100`, `0x80` and `0x1002` had been established
from packages before the source was read; it calls them `DIGESTS`, `METAS`, `GENERAL_DIGESTS`
and `PLAYGO_CHUNK_SHA`. Agreement in that order is worth more than either half alone.

`PLAYGO_CHUNK_SHA` went from opaque to solved: four bytes of SHA-256 per 64 KiB block. The
sizes matched all three packages exactly on the first check - and the *contents* did not, in a
pattern that turned out to be the answer. Eight blocks wrong where the image starts at
`0x80000`, eighty-eight where it starts at `0x580000`: precisely the blocks before the image.
The digests are taken before the body is written, so those early blocks describe a buffer that
no longer exists. Every block from the image onward matched - 655, 951, 1152.

That same circle bit the writer. The first version computed the block digests after writing the
body, which left the `DIGESTS` entry describing a zero-filled table. The test that runs
`selfish derive` against a package `write` produced caught it on the spot, which is exactly
what it was put there for.

**The licences were named but not opened.** `LICENSE_DAT` is a RIF structure of 1024 bytes with
every field stated; a real package's entry `0x400` opens with near-uniform random and holds one
zero byte in 1024, where the plaintext has a 32-byte zero disc key alone. They are encrypted at
rest, so they stay caller-supplied - but the gap is now named and structured rather than 1024
bytes of entropy to stare at.

