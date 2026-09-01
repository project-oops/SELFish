# D036 - Entry `0x80` is a digest manifest over named things, and two of its slots are established


Twelve 32-byte slots, six of them non-zero, and it is **not** a table over entries - the digest
table at `0x1` already does that. It digests specific, named things:

| offset | holds | samples |
|---|---|---|
| `0x00` | `D2 56 01 00`, then zeros, then a small value | identical in 3/3 |
| `0x40` | SHA-256 of the **whole filesystem image**, from the header's offset to end of file | 3/3 |
| `0xC0` | SHA-256 of the **`param.sfo` entry** | 3/3 |
| `0x20`, `0x60`, `0xA0` | digests of something **not present in the package** | unknown |

`0x40` is the one that matters for a writer: it makes the image digest computable, and it is
the thing a package is *about*.

Three slots are left unexplained and are reported that way by `selfish derive` rather than
rounded up. They were checked against every entry, every region of the package file between
every pair of interesting offsets, and the SHA-256 and SHA-1 of all 134 files extracted from
the three packages. None matched. Whatever they cover is not in the package.

**The leading four bytes are recorded as observed and not interpreted.** They are identical in
all three samples while every digest slot differs, so they are clearly not a digest - but "the
same in three files" is an observation, not a meaning, and naming it would be the invention
principle 5 forbids.

`0x400`, `0x401` and `0x1002` remain wholly unknown. All three are dense and high-entropy in
every sample, and none contains the digest of anything tried. `0x400` is 1024 bytes and `0x401`
is 512 - four and two RSA-2048 blocks respectively, which is a shape worth noting next to the
fact that the two *identified* key entries are 2048 and 256 bytes. That is a shape and not a
finding.

