# 2026-09-09 - The confirmation was circular, and the entry that caught it had just written the rule


Worklog 047 recorded the table confirmed at the current generation by a retail vendor game.
Worklog 048 recorded a container mis-attributed to this collection, and drew the lesson: **a
label in a candidate list is not provenance.**

Both were about the same array, and only one of them applied the lesson.

## What happened

obSCEne's probe picks containers to audit from a hardcoded list:

```c
static const obs_audit_candidate_t obs_audit_candidates[] = {
    {"vendor", "/system/vsh/app/NPXS40038/eboot.bin"},
    {"fake",   "/system_ex/app/FAKE00000/eboot.bin"},
    {"vendor", "/system_ex/app/PPSA03416/eboot.bin"},
    ...
```

The `fake` on row two was checked, found to be a hope about a path rather than a record of what
built the file, and written up. The `vendor` on row three - the container whose nine matching
rows the confirmation rested on - was believed.

Then its `ex_info` came back, in a later sweep, and every field is this repository's own default:
`ptype` `0x1`, `paid` `0x3100000000000002`, `app_version` `0x0`, `fw_version` `0x0`,
`npdrm/type` `0x3`. The row for `ptype` in `data/self-format.tsv` reads, in full: *"PTYPE_FAKE.
THIS is what makes a fake SELF fake."*

So the container that confirmed the table is one this table could have produced, and
**a file written from a table agreeing with that table is a round-trip check.** The limit is
restored, D092 is marked reversed with its text kept, and the table header says what was
actually measured.

## Why it got through

Because it was the answer we wanted. The `fake` label was checked precisely because it produced
a claim that would have forced a change to `mkself` - a builder whose output installs and
executes - and that was worth ten minutes of grep. The `vendor` label produced a claim that
closed a chain running back to D084, and it got none.

D092 contains the sentence "an attribution that happens to be convenient deserves the same check
as one that is not", four paragraphs above the place where it did not. The rule was right and
being able to state it did nothing.

The check that works is the one that caught the other row: **ask what the file measures as, not
what the label calls it.** `ptype` is that measurement, which is why the table gives it the note
it does. It was available in the same log.

## What the sweep is actually worth

The honest reading is close to the inverse of the first one:

- The current-generation loader accepts these values. Already known from worklog 040 - a package
  built here installs, mounts, loads and executes. Consistent, not new.
- The containers most plausibly genuine vendor material, six system apps under
  `/system/vsh/app/`, **all diverge** on the same five rows - and none of their `ex_info` blocks
  could be located at `header_size - 0x70` either, so the tail is unconfirmed for them too.
- Every container that matched is one this table could have produced. Every container that could
  not have been produced here is different. Whether that difference is the generation or the
  kind of container is still open, and a **genuine, non-fake application container** is what
  would settle it. None has been read.

## The thing worth carrying

Two entries in one afternoon, on adjacent rows of one array, and the difference between them was
not care - it was which answer was inconvenient. A rule you can state is not a rule you have
applied, and the moment to apply it is when the evidence agrees with you. (D092)
