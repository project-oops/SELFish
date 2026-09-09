# 2026-09-09 - The second reader, and the error that pointed at the wrong thing


Orbistoun took `selfish-elf` and `selfish-nid` as dev-dependencies and ran them beside its own
reader over 29 modules. **All 29 disagreed.** This is the second migration's dividend and it
behaved like the first: obSCEne's found four defects and two were here.

Two more are here. Neither is where the report pointed.

## The 22, and why they read as something else

Twenty-two failed with *"the program header table runs past the end of the file"*. The program
header table was fine.

`ProgramHeadersOutOfBounds` was raised from two unrelated places - `Elf::parse`, where the header
**table** really is out of bounds, and `dynamic_entries`, where a **segment's contents** are not
in the slice. The message describes the first. Faced with 22 failures and that sentence, a
reader goes and looks at the header table, and orbistoun did, and filed the class as a
header-table disagreement with an honest "I cannot tell you which of us is wrong".

Reproduced here in three commands - wrap a module, cut the inner executable out, ask for its
imports:

```
$ selfish elf inner.elf        # reads the header table, prints six segments
$ selfish imports inner.elf    # Error: ProgramHeadersOutOfBounds
```

Neither reader was wrong about the format. The executable inside a signed container is a *view*:
`ehdr` and program headers, with every segment's bytes in the container's entry list. Handed
that alone this crate has not got the bytes; orbistoun tolerates it because it goes back to the
container. `SegmentNotInFile { p_type, offset, size }` now names the segment and says so.

## The constant that was a unit

Every table address on one eboot differed by exactly `0x6bc000` - the holding segment's `vaddr`.
Orbistoun answers in virtual addresses; `Elf::tables` answers in offsets into the byte slice it
returns beside them. Both right, neither documented, so a differential can only report it as six
disagreements and one suspicious constant. The unit is now in the doc. (D095)

## What was written for nobody

`main` returned `Result`, so Rust printed `Error: {:?}` - and every `Display` in these crates,
several of which carry the reasoning behind a decision, had never been seen by anyone. `main`
now prints the error and exits non-zero.

`SegmentNotInFile` is what made it obvious. Its `Debug` is three numbers. Its `Display` is the
paragraph that would have saved a second reader the trip.

That is the same shape as D093's finding two entries ago, where the round-trip warning existed
only in a test comment. **Three times in one day, the fact somebody needed was already written
down somewhere it could not be read.** A test comment, a `Display` impl, a doc that omitted a
unit. Recording a thing is not the same as putting it where the mistake happens.

## Not settled, deliberately

Six eboots where orbistoun finds a vendor dynamic table and `tables()` says `None`. One is
obSCEne's payload, which this repository already settled the other way - it has no vendor tables
at all, which was the answer to REQ-20260909T1250Z-1f74 - so the class is not uniformly ours.
The other five are commercial titles not on this machine. There is a mechanism worth checking
(`Table::Current` finds its segment by which `PT_LOAD` contains `strtab`, and answers `None`
when none does), and it is going back as a request rather than being guessed at here.
