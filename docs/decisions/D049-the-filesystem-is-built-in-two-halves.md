# D049 - The filesystem is built in two halves, because a package has two filesystems built to different rules


The inner one - the files a title is actually made of - is **plain**: unsigned, unencrypted.
That is not a simplification chosen here; `LibOrbisPkg/PFS/PfsProperties.cs` builds it with
`Encrypt = false, Sign = false`, and the images in three real packages agree. The outer one
holds exactly one file, `pfs_image.dat`, and carries the signatures and the encryption.

This mattered more than it sounds. "Write a filesystem" looked like one job gated on
cryptography. It is two jobs, and the larger one - inodes, directory entries, block allocation
- needs no key at all. `crates/selfish-pfs/src/write.rs` does that half and is verified by
reading its own output back through this crate's reader: build a tree, walk it, compare every
file's bytes. A wrong offset, count or block number shows up as a missing file or wrong content
rather than as an assertion about what byte 0x38 ought to be.

