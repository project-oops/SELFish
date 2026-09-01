# D040 - `PLAYGO_CHUNK_SHA` is solved, and material settled what the source left open


The source says the table is `4 * (package_size / 0x10000)` bytes, each slot the leading four
bytes of that block's SHA-256. The sizes match exactly in all three packages - 2652, 3836 and
4960 for 663, 959 and 1240 blocks.

The contents did **not** all match, and the pattern is the finding: 8 blocks disagreed in the
two packages whose image starts at `0x80000`, and 88 in the one whose image starts at
`0x580000`. Those are exactly the blocks *before the image* - `0x80000 / 0x10000 = 8`,
`0x580000 / 0x10000 = 88`. **Every block from the image onward matched: 655, 951 and 1152 of
them.**

The reason is the assembly order the source describes: the digests are taken before the body
and header are written, so the early blocks describe a buffer that no longer exists. They are a
fact about a builder's ordering, not about the file, and nothing can verify them afterwards.
`derive` checks from the image onward only, and says so.

**That circle also had to be broken in the writer.** The first attempt computed the block
digests after writing the body, which left the `DIGESTS` slot describing a zero-filled table -
and the test that runs `derive` against a package `write` produced caught it immediately. The
digests are now taken over the buffer with the image placed and the body still empty, exactly
as the source does it.

