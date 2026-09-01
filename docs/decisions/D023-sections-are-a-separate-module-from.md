# D023 - Sections are a separate module from `dynamic`, because they answer a different question


Two symbol tables exist and they are not the same table. `.dynsym` lives in the vendor segment
and is what a loader resolves against; `.symtab` lives in a section and is the full link-time
set. A finished module usually carries the first and not the second.

They are read by different code paths on purpose. Treating either as the other gives an answer
for the wrong question, and it gives a plausible one: both are arrays of the same 24-byte
entry, so the wrong table parses cleanly and reports the wrong symbols.

Two smaller rules follow from real files rather than from taste:

- **No sections is not an error.** `Sections::parse` returns `Ok(None)`. A stripped module has
  `e_shnum` zero, which is normal; an `Err` there would make every finished module unreadable.
- **`defines` means defined, not mentioned.** An object references every symbol it imports.
  An existence test that counts those reports a module as defining an initialiser it expects
  somebody else to provide, and the builder then emits a tag pointing at nothing.

