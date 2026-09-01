# The filesystem, and the whole chain end to end


`selfish-pfs` reads all three layers - AES-XTS, zlib-compressed blocks, and the filesystem
itself. 57 tests across the workspace, clippy clean under `-D warnings`.

The complete chain runs on a real package: outer container, key derivation, encrypted
filesystem, the compressed image inside it, and the filesystem inside *that*.

```text
   209238  /uroot/eboot.bin
       96  /uroot/sce_sys/keystone
  1328754  /uroot/sce_module/libc.prx
  2930864  /uroot/Media/store_api.prx
```

And the container reader was pointed at that real `eboot.bin` - a genuine vendor-toolchain
artefact rather than something our own builder produced. It parses, and the derived inner
offset lands exactly: 10 entries, so `0x20 + 10 × 0x20 = 0x160`, which is where the executable
is.

### Surprise: homebrew on the current console uses the *previous* generation's container

Thirty-three containers across three packages, and **not one** carries the current-generation
magic. One of the packages is a working homebrew store for that console, so the old format
demonstrably loads on current hardware.

This inverts the guidance from earlier today. The generation split is real - orbistoun observed
the new magic on retail material - but for anything *this* project builds, the previous
generation is the configuration with evidence behind it. (D010)

### Surprise: a "measurement" that could not have failed

The image offset was recorded as a fixed `0x700000`, confirmed by finding high-entropy data
there in all three samples. That is not evidence: in an encrypted package almost every offset
holds high-entropy data.

The real values are `0x80000`, `0x580000`, `0x80000` - and the offset is a header field, sitting
at `0x410` all along. The original search asked whether `0x700000` appeared in the header;
the question that settles it is where the image actually starts and whether *that* is in the
header. (D011)

Two findings in one afternoon that came from pointing code at real material rather than
reasoning about sources. Both were wrong in the same direction: confident, documented, and
built on a check that could not fail.

