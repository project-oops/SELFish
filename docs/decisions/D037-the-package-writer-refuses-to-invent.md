# D037 - The package writer refuses to invent

**Status:** decided
**Date:** 2026-09-26

`selfish_pkg::write::Builder` computes every entry it can derive and refuses to build when a
required entry is neither computed nor supplied, naming each missing one. A computed entry
cannot also be supplied. Every region left blank is reported in `Built::gaps`.

**Why:** a package that is wrong is acted on by the hardware rather than rejected, and
discovering a hole from an install that did not work says nothing about which byte was wrong.
Two sources for one entry is how a digest table stops matching what it describes.

**Rejected:**
- Filling unknown entries with zeros or copies from real packages: invented content.
- Letting a supplied entry override a computed one: silent mismatch between digests and data.
