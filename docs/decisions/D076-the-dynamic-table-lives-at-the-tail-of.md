# D076 - The dynamic table lives at the tail of the vendor segment, not in the image


Status: **decided**, 2026-08-29.

On an ordinary system `PT_DYNAMIC` is a window onto a mapped `PT_LOAD` and the loader reads it
at its virtual address. A console executable does the opposite, and the arithmetic in a real
one is exact rather than suggestive:

```text
PT_SCE_DYNLIBDATA  offset 0x8c130  filesz 0x3760  vaddr 0  ->  ends 0x8f890
PT_DYNAMIC         offset 0x8f450  filesz 0x0440  vaddr 0  ->  ends 0x8f890
```

The dynamic table is the last `0x440` bytes of the vendor segment, immediately after the hash
table, with nothing between them. Both carry no address; neither is placed. `PT_DYNAMIC` is
sized to exactly sixty-seven tags and a terminator, not to a reservation.

That is also why the loader's frames are named the way they are: `preprocess_dt_entries` is
reached from `calcurate_sce_dynlibdata_layout`, because walking the dynamic entries **is** how
the vendor blob's layout is computed. With the table left behind in a `PT_LOAD`, the loader
walks the region it expects, finds no vendor tag anywhere, and reports the first one it needed:

```text
[rtld] ERROR preprocess_dt_entries:9589: does not have DT_SCE_SYMTABSZ or DT_SCE_HASHSZ tabs.
```

naming two tags that were present, correct, and a hundred kilobytes from where it was looking.
So `install` appends the table to the segment, points `PT_DYNAMIC` at it with no address, and
erases the one the linker left behind - walking to the linker's own terminator rather than
clearing the whole reservation, because a script that gives `.dynamic` the rest of its segment
declares a `p_filesz` covering live data.

