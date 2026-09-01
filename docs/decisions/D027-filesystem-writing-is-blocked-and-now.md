# D027 - Filesystem writing is blocked, and now measured rather than asserted


**Filesystem writing is blocked for the same reason package writing is, and it is now measured
rather than asserted.**

The backlog had this as "reading is done; writing is not", which read like effort remaining.
It is not - it is the reader-only-source problem again, and `examples/superblock.rs` puts a
number on it.

Of the 0x400-byte superblock, **38 bytes are named by a cited field**. Across three real
packages, a further **95 bytes are non-zero and unaccounted for**, in sixteen runs, *identical
in all three*:

```
0x0000  0x0008..0x000c  0x001a  0x0028  0x0038..0x003a  0x0052  0x005a  0x0062
0x0068..0x006c  0x0070..0x0074  0x0078..0x007c  0x0080..0x0084
0x00b0  0x00b8..0x00d9 (33 bytes)  0x036c  0x0380..0x03a0 (32 bytes)
```

Two of those runs are thirty-two bytes wide and sit where a digest would. **That is a shape,
not a meaning**, and shape is not something to build on - it is exactly the reasoning D012
gives for the package's six unknown entries.

The inode is worse in proportion: five fields cited out of 0xA8 unsigned, 0x2C8 signed.

A reader walks a filesystem by following four numbers and never looks at the rest. The
container was in this exact position until an open-source *writer* turned up and supplied every
block a reader never reads. The same is needed here, and more effort against reader-shaped
sources does not close it.

The local kit was swept for one and has none. What it does have is **UFS2** - a FreeBSD
`ufs/ffs` header tree and a `ufs2_object.cpp` - which is a *different filesystem*, present
because some homebrew images genuinely are UFS2. Recorded because those headers look relevant
and are not, and the next person to search will find them first.

Recorded so that "reading is done; writing is not" is not mistaken for a task waiting to be
picked up.

