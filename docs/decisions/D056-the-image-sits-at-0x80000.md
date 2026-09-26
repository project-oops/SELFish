# D056 - The image sits at `0x80000`

**Status:** decided
**Date:** 2026-09-26

The package builder places the filesystem image at `0x80000` and writes the whole header
block from `0x400`, including the image size, package size and the constant version date and
hash every real package carries.

**Why:** the header names the image offset, so another offset is legal, but every real package
and `LibOrbisPkg` use `0x80000`, and the reader here is hardware nothing can debug. Matching the
samples is a stronger claim than being legal by the format.

**Rejected:**
- Packing the image directly behind the entries: legal and unobserved, and it saves little.
