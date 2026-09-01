# D026 - The end-to-end test is the point of putting both halves in one crate


`tests/module.rs` links a real object with `ld.lld` through this repository's own linker
script, builds a vendor segment from the linked tables, installs it, and then reads the result
back with `dynamic` - code that knows nothing about how the file was made.

That chain crosses every piece added this session: `layout` (the script), `section` (finding
`.dynsym` and the initialiser), `dynlib` (writing), `nid` (encoding), `dynamic` (reading). Each
has unit tests against synthetic input. This is the one that says they agree about a real file,
which is where the disagreements have historically been.

It also re-checks the adjacency the tag meanings were originally derived from -
`RELA + RELASZ == HASH` - now as a property of *output*. The same arithmetic that identified
the tags checks the writer, and the test fails if neither adjacency was exercised, so it cannot
quietly stop proving anything.

