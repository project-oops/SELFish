# D017 - Relocations are read and censused here; applying them stays with the consumer


`reloc.rs` reads both tables, splits `info` into a type and a symbol index, and says which
types need a symbol and which need thread-local storage. It stops there.

Applying one means computing `symbol + addend` against a chosen base and deciding what to do
when a symbol does not resolve - and the three consumers answer that differently. An emulator
wants a tally of what it skipped so it can say why an image misbehaved; a builder wants a hard
failure. Neither policy is a fact about the format, so orbistoun keeps its `RelocationTally`
and this crate does not grow one.

What did move is the classification, because *that* is format knowledge: a TLS relocation is a
TLS relocation everywhere, and silently skipping one leaves a pointer that looks valid and is
not.

