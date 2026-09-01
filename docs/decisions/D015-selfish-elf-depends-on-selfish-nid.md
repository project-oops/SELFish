# D015 - `selfish-elf` depends on `selfish-nid`, which reorders the spine


The spine was written as `abi, elf, container, nid, pfs, pkg` - nid after elf, because the
hash looked independent of the executable format. It is not. A vendor module's undefined
symbols are named `<hash>#<library>#<module>`, so reading the symbol table and decoding the
hash are the same operation: an ELF reader that stops before decoding hands its caller a
string that every caller then has to take apart the same way.

The spine is now `abi, nid, elf, container, pfs, pkg`. nid still depends on nothing.

The cost is that `sha1` reaches the loader path, and CLAUDE.md promises an emulator reading a
bare executable can take `elf` and stop. It still can - it just gets the decoder with it, and
it needs one, because resolving an import is the reason it read the table.

The alternative was leaving the join to each consumer. Three consumers writing the same twenty
lines is the duplication this repository exists to end, and it would be a *silent* one: each
copy would look correct in isolation, and D016 is what happens when one of them is not.

