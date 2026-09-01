# 4. ~~Writing the filesystem~~ - done, all three layers


A package nests two filesystems and they are built to different rules. Both are written now,
and so is the container between them:

- `selfish_pfs::write` - the **inner** image: superblock, inode table, directory entries,
  block allocation, and the flat path table's real hash index. Plain, because that is what the
  inner image is - `PfsProperties.cs` builds it unsigned and unencrypted and three real images
  agree. (D049, D057)
- `selfish_pfs::pfsc` - the container. **It does not compress**: the block map is absolute
  offsets and a full-size block is stored as-is, which was on the blocker list under the wrong
  description for a long time. (D050)
- `selfish_pfs::outer` - the **outer** image: signed and encrypted, holding the container as
  its single file. Every key comes from the content id and the passcode; nothing has to be
  recovered. (D052, D053)

The 32 bytes at `0x380` are **no longer unexplained**. They are the header's own block
signature - an `HMAC-SHA256` under a key derived from `EKPFS`, followed by the block index, in
the same 36-byte shape as every other block signature in the image. `LibOrbisPkg` writes them
as `BlockSigInfo(0, 0x380, 0x5A0)`. The measurement that called them "33 non-zero bytes of
something" was counting one signature and the first byte of the index after it. (D052)

Still open here: a payload needing a **doubly-indirect** signature block (past roughly 117 MiB)
returns an error rather than a guessed layout, and a **collision resolver** for two paths that
hash alike is not written - `has_collision` answers the question before building and `build`
refuses rather than dropping an entry. (D053, D057)

