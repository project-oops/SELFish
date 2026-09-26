# D105 - Linker scripts keep `.eh_frame`

**Status:** decided
**Date:** 2026-09-26

A linker script here discards only sections that nothing in the linked image reads.
`link/native_eboot.ld` keeps `.eh_frame` and `.eh_frame_hdr`, because a title that links a
freestanding libunwind finds its frame table through `__eh_frame_start` and `__eh_frame_end`. A
title that links no unwinder may drop them in its own script.

**Why:** a `/DISCARD/` entry beats the `KEEP` that brackets those symbols, so the unwinder gets an
empty range and no C++ exception can be caught. The cost is proportional to code that can throw.
Checking the symbols exist is not enough; the range must be non-empty.

**Rejected:**
- Discarding `.eh_frame` everywhere: C++ titles cannot catch.
- Keeping it in the scripts no throwing title uses: they are changed when one is observed to
  need it.
