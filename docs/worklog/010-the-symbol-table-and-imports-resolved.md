# The symbol table, and imports resolved to names


`dynamic.rs` could find the tables; it could not read them. Added `symbols()` - 24-byte
entries, little-endian, bounded by whichever is smaller of the stated size and the segment -
and `imports()`, which walks them, keeps the undefined ones, decodes each name through
`selfish-nid`, and names the library and module each asks for.

That decode is why `selfish-elf` now depends on `selfish-nid` (D015). The two were separate
on the assumption that a hash has nothing to do with an executable format. In this platform
the symbol name *is* the hash, so they were never separable.

### Checked against three real modules

```
eboot.bin       139 imports   88 libc, 13 libkernel, 8 libScePosix, ...
libc.prx        109 imports   96 libkernel, 11 libSceFios2, ...     (of 2,676 symbols)
```

Two things fell out that no synthetic test would have produced.

**Library ids are not positions.** `libc.prx` lists its libraries as 1, 2, 3, 0 - libkernel
last, numbered zero, and carrying 96 of the 109 imports. The first implementation indexed by
position, which would have reported all 96 as `libSceFios2`: a real name, a right count, and
an entirely wrong answer. (D016)

**Library and module are separate namespaces**, and the store's eboot proves it: eight symbols
from library `libScePosix` in module `libkernel`. The POSIX library ships inside the kernel
module. Collapsing the two would have read as correct until exactly this case.

The 2,676-versus-109 gap is the other half of the point: libc *defines* most of its table.
Skipping the section-index test would have reported a library as importing everything it
provides.

`selfish imports <file>` groups by library by default, `--all` lists every one. Container
unwrapping moved into a shared helper so `elf` and `imports` reach the executable the same way.

