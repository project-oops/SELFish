# The package's outer container


`selfish-pkg` reads the header and entry table - 8 tests, and validated against all three real
current-generation packages through `examples/inspect`:

```text
PS5_ITEM00001_v1.14.pkg: 14 entries, 0 of the expected set missing
PS5_LAPY20011_v1.05.pkg: 23 entries, 0 of the expected set missing
  beyond the expected set: 0x1004, 0x1005, 0x1006, 0x100b, 0x100d, 0x1220, 0x1280, 0x12a0, 0x12c0
Store-R2-PS5.pkg: 14 entries, 0 of the expected set missing
```

Structure from cited readers, confirmed by real files, in that order. The samples settled two
things no source states: which entry identifiers a package always carries, and that the image
offset is a convention rather than a field. See D008.

46 tests across the workspace. Clippy clean under `-D warnings`, `unsafe_code` forbidden
everywhere.

### Every parser here refuses a claim it cannot verify

A count is a claim. All three readers bound one against the bytes actually present before
allocating: an entry count of `0x00FFFFFF` in a small file is a parse error rather than a
sixteen-megabyte allocation, and there is a test for each that supplies exactly that.

This is not defensiveness for its own sake - these formats arrive from a console, a download
or somebody else's build, and a library that reads them is the wrong place to learn that
lesson twice.

