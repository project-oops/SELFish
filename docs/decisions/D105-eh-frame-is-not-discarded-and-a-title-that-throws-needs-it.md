# D105 - `.eh_frame` leaves `/DISCARD/`, because a title that throws cannot unwind without it

**Status: decided. 2026-09-23.**

`link/native_eboot.ld` listed `*(.eh_frame .eh_frame_hdr)` in its `/DISCARD/` block. It no
longer does. Every title built against this script that uses C++ exceptions was, until today,
unable to catch one.

## What the discard cost

A C++ title links a freestanding libunwind - `oops-apps/src/oops-deps/libcxx` builds it with
`_LIBUNWIND_IS_BAREMETAL=1`. That macro replaces the `dl_iterate_phdr` lookup, which needs a
dynamic loader this platform does not give a title, with a fixed range named by two symbols:
`__eh_frame_start` and `__eh_frame_end`. `oops-deps/libcxx/link/eh-frame.ld` defines them, by
bracketing a `KEEP(*(.eh_frame))`, and splices itself in with `INSERT AFTER .text`.

**A `/DISCARD/` entry beats that `KEEP`.** The input sections went nowhere, the two symbols
landed on the same address, and libunwind took a zero-byte range as the whole frame table.

The failure that produces is not the one it sounds like. With no frame table the unwinder
cannot step out of the frame that threw at all, so it never reaches the handler tables and
never asks what any `catch` would accept. `oops-utilities/cxx-throw` on hardware terminated
with `libc++abi: terminating due to uncaught exception of type BoundaryError` inside a function
whose `try` has a `catch (...)`. A `catch (...)` that does not catch reads as a personality
routine fault or a broken LSDA; it was neither. `.gcc_except_table` was present and correct the
whole time.

The loader has been reporting this in plain words for as long as it has been true -
`WARNING: corrupted eh_frame_hdr or eh_frame in /app0/eboot.bin` appears in the log of every
title built this way, including ones that run perfectly, because a title that never throws
never reads the table it does not have.

## Why the entry was there, and why that reasoning does not hold

The `/DISCARD/` block drops what a console executable has no reader for: `.comment`,
`.note.*`, `.gnu.version*`. `.eh_frame` looks like it belongs in that list - nothing on the
platform side consumes it, and it is not small. The gap is that its consumer is not on the
platform side. It is *inside the title*, linked from `oops-deps`, and it is the only reader
there will ever be.

So the rule this entry settles is narrower than "keep debug-adjacent sections": **a section
this repository's scripts discard must have no reader in the linked image either.** The other
three entries pass that test; this one never did.

## The cost of keeping it

~19 KiB on `cxx-throw`, which is 19,540 bytes of `.eh_frame` plus 3,580 of `.eh_frame_hdr`.
That is proportional to the code that can throw, so it is roughly nothing on a C title - the
compiler emits no CFI for functions that cannot unwind, and a freestanding C title emits
almost none - and it is unavoidable on a C++ one, where the alternative is a title that cannot
catch.

Nothing about the load is affected: both sections land in the read-only `:data` segment the
script already maps, and neither is referenced by the dynamic tables.

## What the check is, since a clean link is not one

Four `__eh_frame_*` symbols with non-zero addresses is **not** evidence, and that is what made
this last as long as it did - `cxx-throw`'s `make check` asserted exactly that and passed on
every broken build. The symbols were all defined and all non-zero; `start` and `end` simply
held the same value.

The check that means something is the **width**:

    nm <title>.elf | grep __eh_frame_      # end must exceed start, not merely exist

`cxx-throw` now asserts that, reading the link map rather than the ELF because `make title`
rewrites the ELF into a console module and moves its symbol table out of reach.

## Where else this line lived, and why those copies kept it

Four copies of this script exist under `oops-apps`, as `local_tls.ld` - this script plus a
`PT_TLS`, for titles that need thread-local storage. All four carried the same `/DISCARD/`
entry, and the first instinct was to change all four in the same breath. **That was measured
and reversed within the hour.**

`oops-libunwind.mk` is included by `oops-utilities/cxx-throw` and by nothing else. The four
`local_tls.ld` titles - `mesa-demos`, `mesa-cube`, `mesa-dri-probe`, `mesa-winsys-probe` - link
no unwinder at all, which `grep -c libunwind build/mesa-demos.map` answers as `0`. So for them
`.eh_frame` has no reader in the image, and keeping it cost **1,386,512 bytes** on the
mesa-demos eboot: 26,733,824 to 28,120,336, measured by relinking `textures` before and after.

That is the rule above applied honestly rather than a second exception to it. A section is
discarded exactly where nothing in the linked image reads it; the base script cannot know
whether a title links an unwinder, so the base script must keep it, and a title that knows it
does not may drop it. Each of the four now says so, and says what to change if it ever includes
`oops-libunwind.mk`.

`link/eboot.ld`, `link/library.ld` and `link/module.ld` still carry the entry. They are left
alone deliberately: no title built through them has been observed to throw, and changing a
linker script on a reading of the code rather than on a run is the shape of mistake this
repository's second principle exists to prevent. When one of them is used for something that
throws, the same width check will say so in one line.
