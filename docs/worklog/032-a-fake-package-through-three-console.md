# A fake package through three console rejection stages (measured on hardware)


Driven by installing obSCEne on a real PS5 over prosperous's HTTP handover. Each stage was a
distinct rejection with its own error, and each fix was confirmed against a real package before
a hardware trip. The package now clears every check this crate can reason about; the last error
is a shellcore-internal code no available source defines.

### Stage 1 - `0x80f00101`, header integrity (SOLVED)

The header was full of zeros a real one fills, and it is signed. All measured against three real
packages (or reproduced exactly from `LibOrbisPkg@6434772`, cited):

- `content_type`/`drm_type`/`content_flags` were `0` -> `0x1A`/`0x0F`/`0x0A000000`
- `sc_entry_count` = **6** (not the entry count); `main_ent_data_size` = the 5 SC entries' sizes
- `promote_size` = image offset; manifest `0x1C` = `0x6E`; entry table at a **fixed** `0x2A80`
- `sc_entries1/2` (`0x100`/`0x120`), `digest_table_hash` (`0x140`), `body_digest` (`0x160`),
  `pfs_signed_digest` (`0x460`) - all were absent
- **`0xFE0`** = SHA-256 of the header; **`0x1000`** = that header's SHA-256 wrapped under pkg
  public key 3 - the same published-key primitive `wrap_key` already used for the key blobs, so
  no vendor secret (D054's argument). `sig_probe` reproduces a real package's signature exactly.

The playgo block digests (`0x1002`) also digested from byte zero; they skip the header blocks now.

### Stage 2 - `0x80f00200`, content structure (SOLVED)

- The **entry name table** (`0x200`) and **playgo chunk descriptor** (`0x1001`) were empty; both
  are generated now (the chunk descriptor is the full `ChunkDat` structure, not just its header).
- The **playgo manifest** was a wrong one-line body; it is the real `chunk_info`/`scenarios` XML.
- The manifest (`0x80`) ContentDigest/HeaderDigest/MajorParamDigest were blank; all filled.

### Stage 3 - `0x80b211c8` & `0x80f50009` (SOLVED)

The decisive fixes that made packages install on hardware with `error=0x0`:

1. **Entry Table vs Body Layout Order**: The 32-byte table records in the Entry Table at `0x2A80`
   (and entry `0x100`) must be sorted in ascending order of **`entry.id`** (`0x1, 0x10, 0x20, 0x80,
   0x100, 0x200...`). The bodies themselves remain placed starting at `0x2000` in `layout_rank`
   order (`0x10, 0x20, 0x80, 0x100, 0x1, 0x200...`). Sorting the table records themselves by
   `layout_rank` placed `0x10` at index 0 and broke the console's binary search.
2. **Entry `name_offset`**: Byte `+0x04` in each 32-byte table record must point to that file's
   null-terminated string offset in entry `0x200` (`names.bin`).
3. **SC Flags**: Populated standard `flags1` (`0x40000000` for `0x1`/`0x200`, `0x60000000` for
   `0x10`/`0x80`/`0x100`, `0xE0000000` for `0x20`) and `flags2`.
4. **`PARAM.SFO` Alphabetical Key Order**: Sony's `AppDb::AppInfo::ExtractPrimaryKey` binary-searches
   the SFO index table for `TITLE_ID` and `CONTENT_ID`. All 29 keys must be in strict alphabetical
   order, otherwise `ExtractPrimaryKey` fails with `0x80f50009`.
5. **Keystone**: A 96-byte `sce_sys/keystone` is required in the package filesystem image for
   `AppPromoter` to complete package pre-promotion and database registration.
6. **Runtime & Escalation SDK**: Added `runtime/` (`crt0.c`, `escalation.c`, `include/escalation.h`,
   `installer.c`) as the shared SDK layer for homebrew projects building with SELFish.

With these in place, `PlayGoCore::RequestInstall` transfers, pre-promotes, registers, and installs
packages with `STATE_COMPLETE, RUN_STATE_COMPLETE, error=0x0`.

