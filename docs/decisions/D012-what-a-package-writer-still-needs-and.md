# D012 - What a package writer still needs, and what it does not


Writing a package means emitting fourteen entries. Reading was enough to establish which
fourteen; it is not enough to fill them. Characterised by looking at what each holds - sizes,
how much is printable, and whether it announces itself:

| entry | what it is | how known |
|---|---|---|
| `0x10`, `0x20` | the key material | used by the reader already |
| `0x200` | a filename table | 92% printable, names the icon |
| `0x1000` | `param.sfo` | begins with the PSF magic; shadPS4 has a reader |
| `0x1001` | a playgo chunk | begins `plgo`; shadPS4 has a reader |
| `0x1003` | XML | UTF-8 BOM then `<?xml version` |
| `0x1200` | a PNG | the icon entry `0x200` names |
| `0x409` | zeros | 8192 of them in the sample |
| `0x1`, `0x80`, `0x100`, `0x400`, `0x401`, `0x1002` | **unknown** | sizes and entropy only |

So the gap is narrower than "a package is opaque" and wider than "just write the bytes". Six
entries have no established meaning, and two of those (`0x400`, `0x401`) are high-entropy and
plausibly digest tables - plausibly, which is not a thing to build on.

### The lawful route is a writer, and there is not one here

Identifying a PNG by its magic is reading a filename. Working out what belongs in `0x400` by
staring at 1024 bytes of entropy is reverse-engineering, and it produces exactly the
convergence the provenance rule exists to prevent.

This is the position `mkself` was in before an open-source *writer* turned up in the emulator
kit and supplied every block a reader never looks at. The same thing is needed here: a
packaging tool whose source states what it puts in those entries. `LibOrbisPkg` is named by
the extractor this project already cites, and is not in the local kit.

Recorded as a **blocked** item rather than a hard one. The distinction matters: more effort
against the current sources does not close it, and the honest next step is finding a citable
writer rather than trying harder.

Status: **decided** - writing is scoped and blocked on a source, not on work.

