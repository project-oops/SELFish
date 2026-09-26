# D071 - A small inner filesystem is warned about, not adjusted

**Status:** decided
**Date:** 2026-09-26

`selfish pack` warns when the inner filesystem is smaller than `DEFAULT_CACHE_SIZE`. The
header's declared cache size stays the value every real package carries; `Builder::cache_size`
overrides it for a caller that asks.

**Why:** the hardware refuses to mount an inner filesystem below a fixed size, somewhere in
`(720896, 1769472]` bytes, with `Failed to enable GDDR5 cache` and `EINVAL`. Lowering the
declared cache size to the inner size does not change that, so only the caller, by padding the
title, can.

**Rejected:**
- Clamping the declared cache size: tested on hardware and refused identically.
- Refusing to build: a small title is not an error in the builder's input.
