# D078 - A bundled library is a third layout, and it needs a third linker script


`module.ld` and `eboot.ld` differ in two lines: whether the first segment covers the ELF header
and program header table (`FILEHDR PHDRS`), and what address the image is based at. A library a
title bundles in its own package needs **the combination neither of them has** - headers outside
the first segment, image based at zero - so `link/library.ld` is the third.

A system loader verifies the header region before it will load one at all, and refuses a file
whose first segment starts underneath it:

```text
[rtld] ERROR verify_ehdr:930: B: offset 0x0  end 0x190
[rtld] ERROR self_load_shared_object:2816: Unsupported ELF e_type. libc.prx fe18
```

`0x190` is where the program header table ends; `0x0` is where the first segment began. **The
second line is a red herring and cost a build.** `0xfe18` is exactly what a real bundled library
carries - it was measured, from one in a real package - and the loader prints that message after
the header check has already failed. An error naming a field is not evidence about that field,
which is the same lesson D075 records about `Failed to load SCE_DYNLIBDATA`.

The script also drops `PT_INTERP` and the process parameters. A library is loaded by an
executable that has both already; left in, they appear as headers of size zero - an interpreter
whose path is the empty string, and process parameters that are not there.

`ObjectType::SharedLibrary` reaches the builder through `mkmodule --kind shared`, which replaced
a `--fixed` bool: two states for three cases, and the third was not foreseen.

