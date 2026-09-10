# 5. ~~Writing a package~~ - done, image included


`selfish pack --dir <dir> -o <file> --content-id ID` builds one from a directory of files,
whole. **Eight entries are computed**: both digest tables, the digest manifest, the block-digest
table, both licences, and **both key blobs**.

The key blobs were the one certain hardware failure. `0x10` and `0x20` carry what the hardware
unwraps to reach the filesystem, this crate used to demand them, and a caller with nothing to
hand supplied zeros - a package that parses, extracts, passes every test here, and cannot be
opened. They are reproduced **byte for byte** against real packages now: 2048 of 2048 and 256
of 256, on two independent samples. Re-run it:

```
cargo run -p selfish-pkg --example wrap_keys -- <package>...
```

The header past `0x410` was the other one, and was entirely zero - including `pfs_image_size`,
so there was nothing to mount. Measured out of a real package and matching offset for offset.
(D054, D056)

**A caller now supplies nothing.** `pack --dir <tree>` with no `--entry` at all writes a
complete fourteen-entry package. The **entry name table** (`0x200`) is a pure function of which
entries are present, and this crate already knew every name (worklog 064); **playgo-chunk.dat**
(`0x1001`) is fixed for a single-chunk title apart from two sizes, both of which are in hand at
`Builder::build` - the package, and the inner filesystem read out of the `PFSC` header (D099).
`param.sfo` is generated - it is a format, and the field set is measured from real
current-generation packages (D061) - and the icon and the playgo manifest have tool-level
defaults. A supplied entry still wins, for a package being rebuilt to match existing material.

Still open: three slots of `GENERAL_DIGESTS` digest something not present in any package, and
are reported as gaps rather than filled, so `is_complete()` says `false`.

**What the hardware has seen.** A package built entirely through this crate - no `--entry`
supplied, every entry computed, including the `0x1001` inner size (`0x630000`) this crate now
derives from the `PFSC` header - **installs and launches on target hardware**: obSCEne's package
sweep `20260910-161904` ran 54 sections to completion, 174 pass, with
`scePlayGoCoreGetRawContentInfo` accepting it (obscene REQ-20260910T0520Z-9c33, closing the D099
caveat). And a native title built through `--format title --icon` installs and launches with the
converted tile verified on console (sweep `20260910-170907`, obscene REQ-...-2b9f). What remains
unproven is only the three `GENERAL_DIGESTS` slots, which digest something found in no package and
are reported as gaps rather than filled.

