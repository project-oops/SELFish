# 2026-09-09 - The tail held, and the instinct about it was wrong the other way


REQ-20260909T1545Z-2d84 asked obSCEne to stop *testing* the `ex_info` scalars and just report
them. It had been hiding them behind a known-`ptype` check, so nine containers in a row came
back `ex_info | not located` - a sentence that reads as a fact about the format and was actually
a fact about the check. They removed the gate and reported the numbers.

Seven containers that this repository demonstrably did not write - six system apps under
`/system/vsh/app/`, plus `FAKE00000`:

```
paid        0x1bac98  0x7ce04  0x20a0a0  0x176144  0x219d74  0xa36c4  0x8e7a34
ptype       0x0       0x0      0x0       0x0       0x0       0x0      0x0
app_version 0x0       0x0      0x0       0x0       0x0       0x0      0x0
fw_version  0x48      0x48     0x48      0x48      0x48      0x48     0x48
npdrm.type  0x0       0x0      0x0       0x0       0x0       0x0      0x0
```

## The instinct, and why it was wrong

First read: *noise*. Small `paid` values that look like file offsets, a `ptype` of zero that is
not a `ptype`, no npdrm block - obviously the tail is not at `header_size - 0x70` on these, and
the PS4-derived layout is refuted at this generation. That was even the outcome the request had
named in advance as complete and useful, which is its own warning sign.

Then the seven were laid out side by side. **Seven files of different sizes agreeing exactly on
three of four fields is not what a misaligned read produces.** Reading the wrong bytes of seven
unrelated files gives seven unrelated answers. This gives one answer with a single field
varying.

So the offset is very probably right, and these containers carry a *kind this table does not
describe*. `paid` varying per application is what a program authority id does. The layout from a
PS4 writer survived its first contact with material nobody here produced - which is the first
real result this table has from such material, and it is a positive one.

Stated as *consistent with* rather than proven: the alternative was ruled out by argument, not
by measurement.

## Recorded as observed, and not named

`ptype 0x0` is none of the eight values the table's `ptype` group lists. `npdrm.type 0x0` where
the table pins `0x3` reads as *no npdrm control block*, which a system container plausibly has
no use for. Neither gets a row. Principle 5: inventing `ptype system = 0x0` from one console's
system apps is exactly the guess it exists to stop.

## What this entry is really about

Two ticks ago a confirmation was reversed because the convenient reading went unchecked. This
was the same shape with the sign flipped - the *inconvenient* reading, "the layout is refuted",
was the one that arrived first and felt rigorous. It was wrong too.

Being burned by a hopeful conclusion does not make the pessimistic one true. Both times the
answer came from putting the numbers next to each other rather than from deciding what they
ought to say. (D092, D093)
