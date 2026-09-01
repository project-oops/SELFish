# 1. ~~The vendor dynamic table~~ - done, both directions


`PT_SCE_DYNLIBDATA` and the `DT_SCE_*` tags: read by `dynamic.rs`, written by `dynlib.rs`, in
both tag conventions.

**2,200 lines describing one format, twice** - the reason this repository exists - are now one
implementation with tests on both sides of it. `tests/module.rs` links a real object with the
repository's own script, builds a segment, installs it, and reads it back through the reader.
(D025, D026)

What is deliberately not here: which library resolves a given name. That is a manifest, and it
arrives as a closure.

