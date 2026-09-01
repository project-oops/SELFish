# The package header is integrity-protected, and that is the wall (measured on hardware)


A package built here was refused by a console with `0x80f00101` from
`scePlayGoCoreGetRawContentInfo`. Chasing that produced five real fixes to this crate, and then
a result that explains why none of them was enough.

### The fixes, all measured against three real packages

| field | was | now |
|---|---|---|
| `content_type` / `drm_type` (`0x74`/`0x70`) | `0` | `0x1A` / `0x0F` - zero reads as "not a title to register" |
| `content_flags` (`0x78`) | `0` | `0x0A000000` |
| `sc_entry_count` (`0x14`) | the total entry count | `6` - it counts SC entries only, 3/3 |
| `main_entry_data_size` (`0x1C`) | ~520000 (table-to-image gap) | summed SC entry sizes, 512-aligned |
| `promote_size` (`0x7C`) | `0` | the image offset, 3/3 |
| manifest `0x1C` | `0` | `0x6E`, 3/3 |
| entry table offset | `0x1000`, "only has to not collide" | `0x2A80`, a fixed constant 3/3 |
| `playgo_chunk_sha` (`0x1002`) | digested from byte 0 | skips the header blocks; slots 0-7 zero, 3/3 |

Two further entries obSCEne was leaving empty are now generated from measured structure: entry
`0x200` (the NUL-separated entry name table) and entry `0x1001` (`plgo`, 416 bytes, content id
at `0x40`). `derive::playgo_chunk` is the canonical writer for the latter.

### What the bisect found

Copying our bytes into a known-good package one region at a time narrowed the rejection to the
table offset - fixed above. With that corrected the same test moved on, and the decisive
experiment was simpler: **flip one byte of a working package and see what survives.**

| byte flipped | result |
|---|---|
| `0x600`, a reserved area nothing reads | **refused** `0x80f00101` |
| `0x7000`, inside an entry's data | accepted, `0x00000000` |
| `0x80100`, inside the filesystem image | accepted, `0x00000000` |

**The header region is integrity-protected; entry data and the image are not.** A single changed
byte in a region no field uses is enough to refuse a package that otherwise installs, and a real
package truncated to a fraction of its size still installs as long as its header is untouched.

So `0x80f00101` is not a verdict on any field this crate writes. Every value above can be
correct - and now is - and the package will still be refused, because the protection covers
bytes rather than meanings. Nothing this crate can compute produces that, and per principle 6
nothing here will claim to be the vendor in order to try.

This is worth having found: it converts "the package is wrong somewhere" into a bounded
statement about which region is checked and which is not, and it is reproducible in three
commands against any real package.

