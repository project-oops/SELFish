# 2026-09-09 - Both magics are accepted, and the cross-census already contained our own file


REQ-20260909T2125Z-3f9d was mostly a retraction of a guess: obSCEne's Makefile defaults
`EBOOT_GEN` to 5 on the Prospero path and passes it to `mkself --generation`, their eboot leg
runs, so it *looked* as though the measurement already existed. Asked rather than concluded.

It did exist, and twice over:

- `build/eboot.bin` begins `54 14 f5 ee`, built by `mkself --generation 5 --privilege root`,
  installed as `PPSA99980` and launched - **292 checks ran to completion**;
- and the payload leg of the same sweep read it back **off console storage**:
  `/system_ex/app/PPSA99980/eboot.bin`, `magic 0xeef51454`, `ptype 0x1`.

## The part that is genuinely funny

`PPSA99980` is one of the **7 fake/current-magic entries** in the 2x2 that settled the magic
question two entries ago. The cross-census that proved genuine containers carry the current magic
already contained this repository's own output as one of its data points, sitting in the control
row, and nobody on either side noticed.

It does no harm - the control row's job is to show the console does not impose the current magic
on whatever is installed, and one of the things not being imposed on is ours. But it is a good
reminder of how little a container id tells you about what produced a file, which is the same
lesson `PPSA02664` taught this morning from the other end.

## What it changes

Both values are accepted, so the reason the `wrap` default stayed 4 - that it was the only value
shown accepted - is gone.

**The default stays 4 on a different reason: it is a route decision.** The two proofs are on
different delivery routes and neither is on the other's. A *package* built with the previous
magic installs, mounts, loads and executes; a *native title directory* with the current magic
installs and launches. `wrap`'s default serves the package path; the native path passes
`--generation 5` explicitly and never consults it. Moving the default would put the package path
onto a value nothing has accepted *there*, and D082 already records the two as different delivery
routes rather than two spellings of one. (D097)

## Three revisions, one surviving sentence

D097 has been rewritten three times today. The population said *previous*, and that was withdrawn
when its evidence turned out to be a fake container. The population then said *current*, 23 of 23
with a control group - a far better measurement, and also not a reason to move. Now acceptance
says *both*, and the default still does not move, because acceptance turns out to differ by
route.

The only sentence that never needed rewriting is the one the first revision added: **a default
turns on what a loader accepts.** Everything that changed was a population, and the population
was never the reason.
