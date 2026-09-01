# D042 - The filesystem superblock was never a wall - it was 95 unnamed bytes


D027 measured the gap honestly and drew the wrong conclusion from it: 38 of 1024 bytes cited,
95 non-zero and unaccounted for, therefore blocked on a source. The measurement was right. What
was wrong was treating "unnamed" as "unknowable" - the same mistake D033 made about the package
entries, and corrected the same way.

`LibOrbisPkg/PFS/PfsStructs.cs` names the whole header. Every field was then confirmed against
three real images:

| offset | field | in every sample |
|---|---|---|
| `0x00` | version | `1` |
| `0x08` | magic | `0x1332A0B` |
| `0x18`-`0x1B` | fmode, clean, read-only, reserved | read-only set |
| `0x20` | block size | `0x10000` |
| `0x28` | block count | |
| `0x30` | inode count | `4` |
| `0x38` | **data block count** | 655, 951, 1152 |
| `0x40` | inode block count | `1` |
| `0xB8` | inode-block signature | the "33 non-zero bytes" measured before it had a name |
| `0x36C` | unknown index | `1` |
| `0x370` | seed | **zero in all three** |

**The five this crate already knew all agreed with the source**, which is the same pattern the
package entries produced and is worth recording in that order.

Two things fell out that are worth more than the names:

- **`data_blocks * block_size` is the image length exactly** - 655, 951 and 1152 blocks of
  64 KiB. `Superblock::image_len` states it, and a caller handed a truncated image finds out
  there rather than three layers down inside a decompressor.
- **That block count is the same number** at which a package's `PLAYGO_CHUNK_SHA` table starts
  describing the image (D040). Two facts derived from different structures on different days,
  agreeing exactly.

Also worth knowing before relying on it: **the seed is sixteen zero bytes in every sample**, so
the key derivation that hashes it is hashing zeros in every image to hand.

The header proper is `0x380`, not the `0x400` this crate slices. The 32 bytes at `0x380` are
past it and still unexplained; they are no longer counted as part of the superblock.

