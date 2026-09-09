# 2026-09-09 - The table was right, and the limit was the wrong shape


**Withdrawn by [worklog 049](049-the-confirmation-was-circular-and-the.md), the same day.** The
container this entry calls a retail vendor game is a fake one, so the confirmation below is
circular. Kept unchanged: how it went wrong is the useful part, and 049 says how.

`data/self-format.tsv` has carried a bold warning since it was written: derived from a PS4
toolchain and two PS4-generation readers, so **nothing in it is confirmed for the current
generation**, and a file built from it is a hypothesis. D084 built the audit to check it, D085
tried to fetch a container and was on the wrong side of the provenance line, D086 replaced the
fetch with an on-console comparison that carries only a verdict off the box.

Today it got its answer, and the answer is that the table was right all along.

obSCEne's `048-selfaudit`, sweep `20260909-144348`, read `/system_ex/app/PPSA03416/eboot.bin` -
a **retail vendor game at the current generation** - and all nine fixed `self_header` rows
matched. Principle 2 nearly always shows up as a refutation; this is the same principle running
the other way, which is much rarer and worth the entry on its own.

## What the earlier divergence actually was

The previous sweep audited one container, a vendor *system app*, and five of nine differed:
`version` 0x10, `attributes` 0x32, `key_type` 0x10000301, `program_type` 0x3, `flags` 0x52. Read
alone that says the table is wrong at this generation. Read beside the game it says the rows are
not invariants - they describe an **application** container, and a system container is a
different kind of file.

Two of the five explain themselves once you look. `program_type` announces it in its own name.
`flags` was never a constant either: the table's own note derives it as
`0x2 | (signed_block_count & 0x7) << 4`, and `0x52` is that formula at five signed blocks where
`0x22` is two. The other three are recorded as observed and **not** explained - principle 5, an
unexplained value gets named and left alone.

So the header keeps a limit and it is a better one: not "this is the previous console's
container" but "these five rows describe an application container, and the format promises them
of nothing else".

## The surprise, which is a disagreement with the answer

The same sweep read `/system_ex/app/FAKE00000/eboot.bin`, which the probe classed **fake**, and
it carries the system app's five values exactly. The resolution reads that as a container this
collection built, and splits the world into application containers versus "signed system / root
apps".

That cannot be true of anything built here. `Privilege` in `selfish-container` is used in
exactly one place - `constants.paid = privilege.paid(constants.paid)` - and `paid` lives in
`self_ex_info`. The five header fields come from the table for **every** tier, so
`mkself --privilege root` writes `program_type` 0x1 and `flags` 0x22 like everything else. A
container built here would have matched, the way the game did. obSCEne does build at
`PRIVILEGE=root` (its `Makefile:908`), which is what made the tier explanation look right - and
checking what `Privilege` actually does is what killed it.

Either `FAKE00000` is not ours, or something rewrote its header afterwards. The probe's `fake`
label classifies the *file* - a container declares itself fake in a field the format provides -
and is not a record of what produced it.

The confirmation does not depend on which, so it went in and the discrepancy went out as a new
request. A resolved request is never reopened, so a disagreement with an answer is a new
question rather than an edit to the old one - which is the rule doing exactly what it is for.

## The thing worth carrying

**An attribution that happens to be convenient deserves the same check as one that is not.**
"Our own builder emits system values" was a tidy story that closed the loop, and it would have
justified changing `mkself`. Six lines of grep said otherwise. (D092)
