# The reader's refusals, tested


17 tests in `crates/selfish-elf/tests/reading.rs`, against synthetic bytes. `src/lib.rs` went
from **58.49% to 90.00%** of regions covered.

Synthetic rather than linked on purpose: the other integration tests here run a real toolchain
and skip without one, which is right for asserting that the linker script and this parser
agree. It is the wrong shape for the refusals - a linker cannot be made to emit a 32-bit
header, a truncated program header table, or a container. Those are exactly what a reader
meets in the wild, and each is one edited byte from the file beside it.

### The surprise, which is a documentation defect

`Elf::generation` claimed:

> `None` for a byte matching neither, which is a real case: an ordinary ELF carries zero here
> for reasons that have nothing to do with a console.

**That branch cannot be reached for that reason.** `Generation::Previous.abi_version()` is
`0`, so an ordinary ELF's zero reads as `Some(Previous)` - the same answer a genuine
previous-generation module gets. `None` only ever means a byte that is neither `0` nor `2`.

The behaviour is right: zero *is* the previous generation's value. The comment was wrong, and
wrong in the direction that matters - a caller trusting it would have believed this byte
separates an ordinary object from a console module, and used it to route files. What actually
separates them is `has_platform_osabi` and `object_type` together, which is now what the
comment says, with the ambiguity stated rather than denied.

Found by writing the test that asserted the comment and watching it fail. Principle 5 says
nothing is invented; a comment describing a branch by a cause that cannot produce it is the
same failure one level up from the code.

