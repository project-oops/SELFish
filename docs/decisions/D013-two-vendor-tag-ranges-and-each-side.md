# D013 - Two vendor tag ranges, and each side documented only the one it uses


The module and library tables have vendor tags in **both** conventions, but not the same ones:

| | legacy | current |
|---|---|---|
| module info | `0x6100_000D` | `0x6100_0043` |
| needed module | `0x6100_000F` | `0x6100_0045` |
| module attributes | `0x6100_0011` | `0x6100_0047` |
| import library | `0x6100_0015` | `0x6100_0049` |

A reader written from retail material sees only the high range; a writer targeting loaders
emits only the low one. Each documents half, and neither is wrong about its half.

### Found by pointing it at a real module

The first version of this crate put the high numbers in both conventions. Reading a module
carrying 352 entries at `0x6100_000F` and `0x6100_0015`, it reported **zero import libraries**.

Nothing was malformed. Nothing errored. The reader looked where its table said and found
nothing there, which is what a wrong tag number always looks like - an empty result rather
than a failed one. The only reason it was caught is that a module known to import from 352
libraries obviously does not import from none.

Two tests had to be corrected rather than the code, because both encoded the same mistake:
one asserted the conventions *share* those tags. That assertion is now `assert_ne!`, which
makes it a test of the actual distinction rather than a restatement of the bug.

### Why this is exactly the case for the repository

Neither project could have found it alone. The writer's tags are correct for what it builds;
the reader's are correct for what it reads. Only putting them side by side shows there are two
ranges - and only a reader that has to handle both notices when one is missing.

Status: **decided**.

