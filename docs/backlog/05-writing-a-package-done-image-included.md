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

What a caller still supplies: the **entry name table** (`0x200`) and **playgo-chunk.dat**
(`0x1001`), passed empty rather than guessed. `param.sfo` is generated - it is a format, and
the field set is measured from real current-generation packages (D061) - and the icon and the
playgo manifest have tool-level defaults.

Still open: three slots of `GENERAL_DIGESTS` digest something not present in any package, and
are reported as gaps rather than filled, so `is_complete()` says `false`. And **nothing selfish
has produced has been near the hardware.**

