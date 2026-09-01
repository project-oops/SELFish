# An eboot is not the module this project builds, in three separate ways


`/app0` mounts, and a console's `rtld` now reads the eboot and refuses it. Each refusal named the
next difference, and all three are the same *kind* of difference: what a loader that **maps** a
module wants, against what a loader that **executes** one does.

| | module (elfldr, emulators) | eboot in a real package |
|---|---|---|
| container magic | current generation | **previous** - and `selfish wrap` already defaulted to it |
| `e_type` | `0xFE10` executable | **`0xFE00`** fixed-address executable |
| image base | `0` - addresses are offsets, the loader adds a base | **`0x400000`** - mapped where the file says |
| first `PT_LOAD` file offset | `0`, with `FILEHDR PHDRS` | **`0x4000`**, and the headers are in no segment at all |

The first two are fixed: `mkmodule --fixed` writes `0xFE00`, the eboot targets wrap at generation
4, and `module.ld` takes an optional `OBS_IMAGE_BASE`.

**The fourth row is the one still standing**, and it is the interesting one because this script
argues for the opposite in a comment that is correct about the case it was written for:

> FILEHDR and PHDRS here rather than on a segment of their own: without them the headers are not
> covered by any segment, and a loader that maps only what the segments describe cannot read the
> header table it just used.

True of a loader that maps segments and then reads the file through the mapping. A system loader
reads the file directly, and refuses one whose first segment starts at file offset zero:

```text
[rtld] ERROR verify_ehdr:930: B: offset 0x0  end 0x190
```

`0x190` is exactly `ehdr + 6 phdrs`. A real eboot puts its first `LOAD` at file offset `0x4000`
with `PT_INTERP` at the same place, so the header region is covered by nothing. Both layouts are
right for their loader, which is why this cannot be a fix to `module.ld` - it needs a second
script, or a switch as deliberate as `OBS_IMAGE_BASE`.

### Worth saying about the method

Every one of these came from measuring a real package's eboot rather than from reasoning about
what a loader ought to want, and three times the reasoning would have been wrong: a
current-generation console wants the *previous* generation's container, a fixed-address executable
is a different `e_type` rather than a linker flag, and mapping the headers - which one loader
requires - is what another refuses. The file was sitting inside a package that has been on the
bench all along.

