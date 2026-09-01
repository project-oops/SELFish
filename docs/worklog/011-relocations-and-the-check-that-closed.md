# Relocations, and the check that closed the loop


`reloc.rs`: `Elf64_Rela` as the ABI defines it, the type constants that appear in these
modules, and `census()`. `dynamic::relocations()` pulls both tables out of the vendor segment
and returns them as a named pair rather than a tuple, because swapping data relocations with
PLT slots gives an image that relocates cleanly and jumps to the wrong place.

The tables have been readable for a while in principle. What was missing was any way to tell
whether they had been read *correctly* - and the two halves of this crate turned out to check
each other.

### Every PLT slot names an import, across eight modules

A procedure-linkage slot exists because a function is imported. So every entry in `DT_JMPREL`
should name a symbol that the symbol-table walk classified as an import, and the two paths
reach that conclusion through entirely different fields: one through `st_shndx` and a decoded
name, the other through the high half of `r_info`.

```
eboot.bin        126 of 126 PLT slots -> imports (of 139)
libc.prx         107 of 107                      (of 109)
libSceFios2.prx   78 of  78                      (of  81)
curl.prx          44 of  44                      (of  44)
sqlite.prx        43 of  43                      (of  53)
jb.prx             3 of   3                      (of   3)
piglet.prx       157 of 157                      (of 160)
store_api.prx    114 of 114                      (of 114)
```

Eight for eight. The gap between the two columns is imported *data*, reached by an ordinary
`64` or `GLOB_DAT` relocation instead of a jump slot - and widening the check to both tables
accounts for every remaining import in seven of the eight.

### The two that are not accounted for

`piglet.prx` imports two symbols from `libc` that **no relocation in either table references**:
`P330P3dFF68` and `H8AprKeZtNg`. Neither matches any name in the local corpora, and neither
matches the obvious guesses (`__cxa_*`, `__stack_chk_*`, `__tls_get_addr`, `atexit`,
`environ`, the C++ operators).

Recorded rather than explained. An import nothing references is normal enough in ordinary ELF
- a linker keeps a symbol a dead code path once used - but "normal enough" is a guess, and the
honest state is two unexplained entries out of 598 across eight modules.

### On `info`, which is packed the opposite way to the obvious reading

The **type is the low half** and the symbol index the high half. Reading them the other way
round gives symbol indices in the thousands and types that are all zero, which presents as an
image with nothing to relocate rather than as an error. There is a test that states it.

