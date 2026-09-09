# D095 - A second reader found three disagreements, and only one of them was about the format

**Status: decided. 2026-09-09.**


**Orbistoun ran `selfish-elf` beside its own reader over 29 modules. All 29 disagreed. The
useful part is that the three classes are three different kinds of thing, and only one is a
question about the container format at all.**

This is the second migration's dividend, and it arrived the way the first one did: obSCEne's
migration found four defects and two were here. This one found two more, and neither is where
the report first pointed - which is itself the finding.

## Class 2, the largest: one error wearing another's name

Twenty-two of twenty-nine failed with *"the program header table runs past the end of the
file"*. The program header table was fine.

`ElfError::ProgramHeadersOutOfBounds` was raised from two unrelated places: `Elf::parse`, where
the header **table** really is out of bounds, and `dynamic_entries`, where a **segment's
contents** are not in the slice. Its message describes the first. A reader with 22 failures and
that sentence has every reason to go and look at the header table, and orbistoun did, and
reported the class as a header-table disagreement.

Reproduced here in three commands: wrap a module, cut the inner executable out of the container,
and ask for its imports. `selfish elf` reads the header table and prints six segments; `selfish
imports` refuses. So there was never a disagreement about parsing at all.

**And the substantive answer is that neither reader is wrong.** The executable inside a signed
container is a *view* - `ehdr` and program headers, with every segment's bytes held in the
container's own entry list. Handed that view on its own, this crate does not have the bytes and
says so; orbistoun tolerates it because it goes back to the container for them. Two API
contracts, both honest. `ElfError::SegmentNotInFile { p_type, offset, size }` now says which
segment and where its contents claim to be, and its `Display` says to read the file through the
container instead.

## Class 4: a units difference, not a parse difference

Every table address on one eboot differed by exactly `0x6bc000`. That constant is the holding
segment's `vaddr`, and the two readers answer in different units: orbistoun returns virtual
addresses, `Elf::tables` returns **offsets into the byte slice it hands back beside them**. Both
are right. Neither documented it, so a differential could only report it as six disagreements
and one suspicious constant.

`Elf::tables` now states the unit in its own doc. That is the whole fix, and it is worth a
numbered entry because an undocumented unit is indistinguishable from a bug to everyone except
the person who wrote it.

## Class 3, which is not settled here

Six eboots where orbistoun finds a vendor dynamic table and `Elf::tables` answers `None`. One of
the six is obSCEne's payload, and that one this repository has already settled in the other
direction: it carries no vendor tables at all - `0 vendor import entries, 0 DT_NEEDED`, no string
table - which was the answer to REQ-20260909T1250Z-1f74. So the class is not uniformly a defect
here.

The other five are commercial titles in orbistoun's corpus and not on this machine, so this
entry does not guess. There **is** a mechanism worth checking when material is available: under
`Table::Current`, `tables()` finds the holding segment by looking for the `PT_LOAD` whose vaddr
range contains `strtab`, and returns `None` when none does. A module detected as current whose
tags actually hold offsets would fail that search silently. Filed as its own request rather than
resolved by reasoning.

## The one that was nobody's class

Every `Display` in these crates was written for nobody. `main` returned `Result`, so Rust printed
`Error: {:?}` and the explanations - several of which carry the reasoning behind a decision - had
never been seen. `main` now prints the error and exits non-zero. `SegmentNotInFile` is what made
it obvious: its `Debug` is three numbers, and its `Display` is the paragraph that would have
saved a second reader the trip.
