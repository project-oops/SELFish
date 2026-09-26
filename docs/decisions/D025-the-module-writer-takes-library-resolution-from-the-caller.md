# D025 - The module writer takes library resolution from the caller

**Status:** decided
**Date:** 2026-09-26

`selfish-elf::dynlib` writes the vendor dynamic tables and edits a linked module to carry them.
Which library resolves a given symbol arrives from the caller as a closure, and an undefined
symbol the closure does not claim is an error naming it.

**Why:** which library provides a name is a manifest, and every consumer has a different one;
none disagrees about the format. Library zero is a real id, so defaulting an unclaimed symbol to
it produces a module that loads and resolves nothing.

**Rejected:**
- A manifest in this crate: it is a consumer's knowledge, not the format's.
- A default library for unclaimed symbols: a valid-looking answer that is wrong at run time.
