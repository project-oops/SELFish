# Package header: sensible defaults instead of zeros (found on hardware)


`selfish pack` wrote drm_type, content_type and content_flags as zero, because the writer
treated them as the caller's and defaulted nothing. A console's installer reads content_type=0
as "not a title I will register" and reports empty content id/type/platform for the whole
package - the exact failure a fake package hit on a real PS5 (obSCEne's install loop).

Fixed per the principle that a builder producing a non-installable package by default is worse
than one with deterministic defaults: `Builder::new` now sets drm_type=0x0F, content_type=0x1A
(CONTENT_TYPE_GD), and `header_value::CONTENT_FLAGS=0x0A000000` - the values a real homebrew
title carries, observed in a working package on a console (oracle, not source). `kind` still
overrides for non-application content.

### Still open: three manifest digests (entry 0x80)

The package is now structurally identical to a real one - same 14 entry ids, same header layout.
The remaining difference is in the manifest (entry 0x80): it holds SHA-256 digests every 0x20
bytes, and selfish fills 0x40 (the image) and 0xC0 (param.sfo) but leaves 0x20, 0x60 and 0xA0
blank. A real package fills them; matching them against every package region, entry, header and
table finds no match, so they are digests of the **plaintext** pfs layers (inner image / PFSC /
outer-before-encryption), which are not stored in the package but are built here. Threading
those digests into the manifest is the remaining work to make a package a console installs.
Confirmed offset 0x80 is zero in a real package too, so that slot is correctly left blank.

