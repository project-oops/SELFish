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

---

## Closed, 2026-09-09: no reader defect existed, and one endorsement here was unfounded

Class 3 is settled, and not by the mechanism guessed at above. Orbistoun re-ran against `main`
with the splice and reported **29 modules, 0 disagreements** - every class gone, including this
one (REQ-20260909T1730Z-5f28).

The cause was neither of the two branches this entry named. It was two more harness comparisons
of unlike things:

- `vendor_tables` against `table.is_some()`. Theirs means *the values it holds came from vendor
  tags*; ours means *which convention the file follows*. Under `Current` the standard tables come
  from standard tags, which is exactly when theirs answers `false`. Compared against
  `Some(Table::Legacy)`, every module agrees.
- `dynamic_bytes` against `tables()`. Theirs finds `PT_DYNAMIC`; ours finds resolvable vendor
  tables. A plain freestanding ELF has the first and not the second - which is obSCEne's payload,
  the one this repository had already settled from its own side.

**Four rounds of reported disagreements, four harness errors, zero reader defects.**

## The correction owed

In the request that produced this, SELFish told orbistoun: *"on the tag convention in that same
module - you are probably right to suspect yourselves"*. That was wrong, they withdrew it, and
the endorsement was ours.

Nothing was checked before agreeing. Their report said "this would make it a defect in **this**
repository, and it is the one I am taking away to investigate", and this repository replied that
they were probably right - about a field whose meaning it had not looked up. Reading the two
field definitions would have shown they were not the same question, which is the whole answer.

That is the third time in one day: a convenient attribution taken on trust (D092), a pessimistic
reading taken on instinct (worklog 051), and now somebody else's self-doubt agreed with for free.
**The failure is not optimism or pessimism. It is answering from what a claim sounds like rather
than from the thing it is about**, and agreeing costs exactly as much as disagreeing when neither
is checked.

## What the clean result is worth

More than a defect list would have been. Two readers built from the same knowledge in different
languages - orbistoun's `dynamic.rs` and `reloc.rs` are what `selfish-elf` was built *from* -
now agree on every field this test compares across 29 modules. The four harness corrections live
in the test's own comments rather than only in an inbox, which is what stops the next run
rediscovering them.

The one finding that survives on this side is the error message, and orbistoun named the rule it
breaks better than this entry did: *a message naming a cause must come from the branch that
determined it*.
