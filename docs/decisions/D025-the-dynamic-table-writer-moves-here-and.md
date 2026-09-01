# D025 - The dynamic-table writer moves here, and the manifest stays with the caller


`dynlib.rs` is the writing half of `dynamic.rs`: it rebuilds the string, symbol and hash
tables, re-encodes every import, emits the tag list in both conventions, and performs the
module surgery a linker cannot - appending the segment, repurposing the declared vendor
header, overwriting the standard dynamic table, and removing the section headers.

**What did not move is which library resolves a given name.** That is a manifest, not a fact
about the format, and it arrives as a closure. Every consumer has a different one; none of them
disagrees about the format.

Returning `None` from that closure for an undefined symbol is an **error naming the symbol**,
not a default. Library zero is a real id - usually the kernel's - so defaulting to it is a
valid-looking answer that resolves to nothing at run time.

The knowledge in this module was earned by obSCEne against real loaders and is transcribed
with its reasoning intact, because the reasoning is the part that stops it being re-litigated:

- The **string table is declared first**. A loader resolves a name offset the moment it meets
  one, and four tags carry name offsets. Emitted earlier they dereference a base it does not
  have - a fault inside the loader before a guest instruction runs, with nothing in its log.
- Imports are **typed as functions**. A loader matches on hash *and* type; an untyped import
  binds to a stub returning zero, so the module runs and every answer is a plausible nothing.
- An **empty relocation table is not declared, but its form always is**. Declaring an empty one
  gives `JMPREL` and `RELA` the same offset and a zero size, and a loader handed that read
  relocations out of the string table.
- A library is **named twice** - `libkernel` for the vendor tags, `libkernel.prx` for
  `DT_NEEDED` - because a loader keying its implementation table on the filename finds nothing
  under the bare name.
- The **initialiser is absent rather than zero**. One loader calls the address without checking
  the tag was present, and a zero there executes the ELF header as instructions.

