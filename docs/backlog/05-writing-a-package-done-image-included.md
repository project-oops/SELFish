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

**What the hardware has and has not seen.** A package built through this crate installs and
launches - obSCEne's `pkg` leg runs its probe from one, and its `000-boot` checks pass
(`reports/hardware/20260909-234847-pkg.obs.log`). What has *not* been on hardware is the
`0x1001` inner size this crate now computes: obSCEne's own packages carried a hardcoded
`0xAA0000` there, which is larger than the package containing it, and the measured `0x630000`
that replaced it is so far justified by arithmetic alone. Filed as obscene
REQ-20260910T0520Z-9c33; D099 records the limit.

