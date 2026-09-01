# D038 - What the cryptographic entries are not, recorded so nobody re-runs the searches


`0x400`, `0x401` and `0x1002` resisted every local hypothesis. Refuted across the samples:

- **Neither RSA key unwraps them.** Both keypairs from the public fake keyset were applied to
  every 256-byte block of all three entries. No block yields PKCS#1 padding, so they are not
  wrapped under the keys that unwrap the entry-keys blob and the image key.
- **They contain no digest of anything available**: not of any entry, any region of the package
  file between any pair of interesting offsets, the filesystem image or any prefix of it, or
  any of the 134 files extracted from the three packages - by SHA-256 or SHA-1.
- **None is a constant.** No byte-identical pair across three titles (D033).

The same treatment was given to the two unexplained 32-byte fields in the PFS superblock, at
`0xb8` and `0x380`. Refuted: the superblock with the field zeroed, the whole superblock, the
seed, the image after the superblock, the whole image, the prefix before the seed, the
following block - under SHA-256, and under HMAC-SHA-256 keyed by the seed.

That is a wall of negatives and it is worth the space. Every one of them is a search somebody
would otherwise repeat, and knowing that the content is **not present in the package** is what
says the answer has to come from a source rather than from more staring.

The source is named and already cited here: `data/pkg-keys.toml` records that the fake keyset
comes from **LibOrbisPkg**, an open-source *packaging tool*. A writer is exactly what unblocked
the container (D012), and this repository already trusts that project for its keys.

