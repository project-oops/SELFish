# The dynamic-table writer, and the loop closing


The largest item on the backlog and the one it was written for: 1,446 lines of writer in
obSCEne against 754 of reader in orbistoun, describing one format, with nothing connecting
them. The reading half came over earlier this session. This is the writing half.

`dynlib.rs` rebuilds the string, symbol and hash tables, re-encodes every import as
`<hash>#<library>#<module>`, emits the tag list in both conventions, and does the surgery a
linker cannot: append the segment, repurpose the declared vendor header, overwrite the standard
dynamic table, strip the section headers.

### What stayed behind

Which library resolves a name. That is a manifest and it arrives as a closure - every consumer
has a different one and none of them disagrees about the format. An unclaimed import is an
error that **names the symbol**, because library zero is a real id and defaulting to it is a
valid-looking answer that resolves to nothing.

### One duplication found on the way in

The writer needs to encode a library id into a symbol name, and the first version carried its
own copy of the 64-character alphabet. That is the exact thing this repository exists to stop.
It is now `selfish_nid::encode_index`, sitting beside the decoder that reads it back, with a
round-trip test.

### The loop closed

`tests/module.rs` runs the whole chain:

```
link/module.ld -> ld.lld -> .dynsym/.rela/.got -> dynlib::build -> install -> dynamic::imports
```

and asserts the import comes back out with the right hash and the right library, that the
section headers are gone, and that `RELA + RELASZ == HASH` still holds - the same adjacency the
tag meanings were derived from, now checked as a property of output. The test fails if neither
adjacency was exercised, so it cannot quietly stop proving anything.

Every piece added this session is on that path: `layout`, `section`, `dynlib`, `nid`,
`dynamic`. They had all been tested against synthetic input. This is the first thing that says
they agree about a real linked file.

