# D056 - The header past `0x410` was entirely zero, so nothing could be mounted


**The header past `0x410` was entirely zero, and a package whose header says its image is zero
bytes long has nothing to mount.**

This crate wrote four header fields: the magic, the entry count, the table offset and the image
offset. A real package's header carries far more, and the block at `0x400` is the part a
console uses to find and size the image - `pfs_image_size`, `package_size`, `pfs_flags`,
`pfs_signed_size`, `pfs_cache_size`, and two digests over the image.

All of it was measured out of a real package with `xxd -s 0x400` and matches `LibOrbisPkg`'s
writer offset for offset. Two constants settle it beyond coincidence: a version date of
`0x20161020` and a version hash of `0x1738551`, both appearing verbatim in the sample and in
the source. A generated header's first sixteen bytes and its whole `0x400` block are now
byte-identical in shape to a real one, and `package_size` is the file's exact length.

The image also moved to `0x80000`. Packing it in behind the entries was legal by the format -
the header names the offset - but every real package and `LibOrbisPkg` both put it there, and
"legal by the format" is a weaker claim than "identical to every sample" when the reader is a
machine nobody here can attach a debugger to.

**That move broke the block-digest table**, which had been sizing itself from the old computed
offset. The test that re-runs the derivation against this crate's own output caught it
immediately, which is the entire reason that test exists.

