# Closing three gaps the migration opened


**A code path that had never run.** `Elf::tables`'s current-convention branch was written from
reasoning and executed against nothing - every module here is previous-generation. That is the
defect class this project keeps finding in other people's work.

It cannot be fixed with material nobody has, but it can be fixed with the other half of the
crate. `tests/current.rs` builds the same linked object under **both** conventions and asserts
that every tag number differs and every import that comes back out is identical. It also pins
the one row in `data/library-versions.tsv` end to end - the display library read back out of a
built module rather than out of the function that decides it.

Three tests, passing first time, which says the reasoning was right. The doc comment now says
exactly how far it has been checked and where it stops, so the limit travels with the
capability. (D030)

**A tool that panicked on a closed pipe.** `selfish imports big.prx | head` ended in a
backtrace. Hit repeatedly while checking this repository's own work, which is the tell: it is
not cosmetic in a binary whose purpose is being pointed at real material. Output now goes
through a `say!` macro that exits zero when the reader has gone. (D031)

**Two CI workflows, one of which cannot pass.** selfish now has one, and it installs a linker
and then *checks the linker tests did not skip* - otherwise a missing toolchain leaves the job
green having verified nothing. obSCEne's cannot pass, because its `tool/` now needs two sibling
checkouts and every job does one bare checkout. Flagged in place with the shape of the fix
rather than restructured blind. (D032)

