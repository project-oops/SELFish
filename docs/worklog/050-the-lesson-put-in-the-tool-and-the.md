# 2026-09-09 - The lesson put in the tool, and the reversal made airtight


Worklog 049 reversed a confirmation: the container that matched all nine fixed header rows
declares itself fake, so the match was a round trip. The rule got written down twice that
afternoon and applied once. Writing it down a third time was not going to help, so it went into
the tool instead.

## The guard

`selfish audit` now reads `ex_info.ptype` at the offset the table pins and prints what kind of
container it read **above** the row count:

```
generation  Current (from the magic)
kind        ptype 0x1 - fake
            this container declares itself FAKE - it agrees with the table because it was
            written from one, so a match is a round trip and says nothing about vendor material

confirmed   9 of 9 fixed header row(s) match the table
```

Above, not below, because `9 of 9` is the number a reader keeps and a caveat under it is a
footnote. A footnote is exactly what failed.

The value is **reported, not tested**: `Declared::Ptype { value, known }` carries the raw number
and the name the table gives it, if any, and `Declared::Unreachable` is a separate answer. Using
"is this a *known* ptype" as the test for "did I find the tail" is what produced nine
`ex_info | not located` lines in one sweep - it fails on precisely the material worth having,
and then reads as a fact about the format. `is_round_trip()` is false for an unknown value on
purpose: *not known to be fake* is not *known to be genuine*. (D093)

## What the crate already knew

`a_container_this_crate_builds_confirms_every_fixed_row_of_the_table` has been in the suite
since D084, and its own comment calls the thing it asserts a round trip. The knowledge was
written down in a place that runs on every commit and never reaches a person reading output.
That is the more useful half of this entry: **a fact recorded only in a test comment is not
recorded where it is needed.**

## The reversal, made airtight

Wrapping a module and reading its tail gives, byte for byte:

| field | this crate's output | `PPSA03416` as measured |
|---|---|---|
| `paid` | `0x3100000000000002` | `0x3100000000000002` |
| `ptype` | `0x1` | `0x1` |
| `app_version` | `0x0` | `0x0` |
| `fw_version` | `0x0` | `0x0` |
| `npdrm/type` | `0x3` | `0x3` |

Five of five. The container reported as a retail vendor game has the exact `ex_info` this crate
writes. Whether it came from here specifically or from another OpenOrbis-lineage toolchain is
still not knowable from here, and does not matter: it is not vendor material, and D092 stays
reversed.

## Not done, deliberately

The audit does not refuse a fake container, warn on stderr, or change its exit code. Auditing
one is legitimate - it is principle 4's round trip and a test depends on it. The defect was
never that the round trip happened, only that the output would not say which one it was. A tool
that turned this into an error would be answering a complaint nobody made.
