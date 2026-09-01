# Section headers and the link-time symbol table


`section.rs`: the 64-byte section header, the name table, section contents, and `.symtab` with
its strings reached through `link` rather than by the name `.strtab` - an object can carry more
than one string table and the name of the right one is not guaranteed.

This is the builder's half of the ELF work. A finished module has no sections at all; a
*linked object* does, and a builder has to look inside one to decide what to emit - whether the
module defines an initialiser, for instance, which decides whether an initialiser tag belongs
in the dynamic table.

### Checked against the toolchain

A real `eboot.bin` reports **no sections**, which is the correct answer and the one that would
have looked like a bug without saying so. A linked ELF on this host reports 43 sections and
14,667 `.symtab` entries, of which 14,593 are defined here. `readelf` on the same file:

```
Symbol table '.symtab' contains 14667 entries
UND entries: 74            (14667 - 14593 = 74)
```

Exact agreement on both numbers, against a completely independent implementation.

### A zero offset is not an absence

The first version stored the name table's location as a `usize` with zero meaning "no name
table". Zero is a legitimate file offset, and the fixture put the name table there - so every
section came back unnamed. Now an `Option`.

Worth recording because it is the same shape as the mistakes this repository keeps finding: a
sentinel that collides with a real value fails silently and plausibly. It was caught by a test
that only exists because the fixture happened to be laid out that way; the test now says so.

