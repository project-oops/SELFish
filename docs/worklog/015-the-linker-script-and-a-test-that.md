# The linker script, and a test that actually links


`link/module.ld` moved over from obSCEne, along with the comments that make it worth having:
every rule in it was learned from a module a loader rejected, or accepted and then read
wrongly.

The three that cost the most:

- **Two loadable segments, not three.** A separate read-only segment is the obvious layout.
  One loader maps segments by kind and silently omits the third, so every `.rodata` byte -
  section tables, check tables, every string - is unmapped, and the module faults on its
  first act. A minimal module survived it, which made the fault look like a problem of scale.
- **`.got` and `.got.plt` stay separate.** Merged, the linkage-table base names the start of
  `.got`, and every resolved import lands at the wrong offset. Presents as a loader that does
  not support the format.
- **16 KiB alignment**, the console's granularity rather than the host's 4 KiB. A segment
  starting part-way into a page faults *inside* the loader rather than being rejected.

### Testing something a compiler never sees

Two layers. `layout.rs` compiles the script in and asserts its text names the same constants
`selfish-elf::segment` does - drift there produces a module whose headers are wrong and whose
build succeeds. Then an integration test runs a real `ld.lld` over it and parses the output
with this crate:

```
LOAD R E, LOAD RW           two loadable segments, not three
DYNAMIC, INTERP
LOOS+0x1000001              PT_SCE_PROCPARAM
LOOS+0x1000000              PT_SCE_DYNLIBDATA
```

It skips when `clang` and `ld.lld` are absent rather than failing - they are not build
dependencies here, and a test that fails on a clean machine teaches people to ignore failures.

