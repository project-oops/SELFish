# The dynamic table, and the whole stack on genuine material


`selfish-elf` now reads the table a loader actually reads: both tag conventions, the string
and symbol tables, and the vendor import lists. 65 tests across the workspace, clippy clean
under `-D warnings`.

`Container::to_elf` completes the pair - reassembling a scattered executable is the inverse of
building one, and both live in the crate together so neither can drift.

The stack runs end to end on a genuine vendor library:

```text
package -> filesystem -> libc.prx -> container -> ELF -> dynamic table
  49 dynamic entries, legacy convention, 2676 symbols
  libSceFios2, libSceLibcInternalExt, libSceSysmodule, libkernel
```

Every container the reader had seen before this was one its own writer produced.

### Surprise: there are two vendor tag ranges, and each project knew one

The module and library tables have vendor tags in **both** conventions - different ones.
Legacy puts them at `0x0D`-`0x19`, current at `0x43`-`0x49`. A reader built from retail
material sees only the high range; a writer targeting loaders emits only the low one.

The first version of this crate used the high numbers for both. Reading a module with 352
import libraries, it reported **zero** - no error, no malformed data, just an empty result,
because that is what looking up the wrong tag number always produces.

Two tests had to be corrected rather than the code: both asserted the conventions share those
tags. One is now `assert_ne!` and tests the distinction rather than restating the bug. (D013)

**Neither project could have found this alone.** Each side's numbers are right for what it
does. Only putting them together shows there are two ranges - which is the clearest
justification this repository has produced for its own existence.

