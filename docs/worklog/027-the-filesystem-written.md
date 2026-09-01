# The filesystem, written


The whole nest can now be built as well as read: a plain inner filesystem, a `PFSC` container
around it, and a signed and encrypted outer filesystem holding that container as its single
file. `outer`'s tests run the round trip end to end - build, wrap, sign, encrypt, then decrypt,
decompress, walk, compare - and the encryption written here is checked against the decryption
that was already in the crate, which is two implementations of XTS rather than one asserted
twice.

### Surprises, which are the point of this file

**"Write a filesystem" was two jobs, and the big one needed no key.** The inner image is plain.
Everything that looked like it was gated on cryptography - inodes, dirents, block allocation -
was gated on nothing. (D049)

**`PFSC` does not compress.** It was on the blocker list as "implement zlib". The block map is
absolute offsets and a full-size block is stored as-is. (D050)

**Writing found two fields reading could not.** `0x368` and `0x36C` are different fields used by
unseeded and seeded images; both had been measured as "a 1 near the end". And `N_BLOCK` is not
the block count - the source writes a literal `1`. Writing the true count would have looked
more correct and made every image differ from every real one. (D051)

**Two independently-derived offsets met in the middle.** This crate's reader had measured a
signed inode's block number at `0x84`. The source computes the signature slot at `0x64`. They
are the same structure: 32 bytes of digest, then the block number. Neither was adjusted to fit
the other.

**The key that signs the filesystem is computed, not recovered.** After a long stretch of
establishing which keys are genuinely unobtainable, this one is a hash of two things the builder
chooses. `examples/filesystem_key.rs` checks the computed key against the one recovered from
real packages: two of three agree exactly, and the third differs only in its passcode - its
image still opens under the recovered key. Three of three open. (D052)

Real packages also confirmed the outer layout independently: every one holds **exactly one
file**, which is what the builder emits.

### Still open

- A payload past twelve direct blocks plus one block of signatures needs a doubly-indirect
  block, and that returns an error rather than a guess. (D053)
- The flat path table is written as a plain list of paths under the right name, with a real
  inode and real blocks. The genuine table is a hashed index and its layout is not established
  here; nothing reads it back, since directory entries are the authority. (principle 5)
- Three `GENERAL_DIGESTS` slots still digest something not found anywhere in a package.
- **Nothing this repository has produced has been near a console.**

