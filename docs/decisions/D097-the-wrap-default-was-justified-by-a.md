# D097 - The wrap default was justified by a population counted in the wrong place, and the default is right anyway

**Status: decided. 2026-09-09.**


**`selfish wrap` defaults to the previous generation's magic. The reason given for that was a
count of containers that are all of homebrew lineage, and a hardware citation that turns out to
name a fake container. The default does not change; its justification does.**

Two places said the same thing:

- `data/self-format.tsv`, on `current_magic`: *"a current-generation title's eboot carries
  previous_magic, measured on hardware (obscene 048-selfaudit, PPSA02664)"*, and on
  `previous_magic`: *"what a current-generation title's EBOOT actually carries
  (hardware-confirmed)"*.
- `selfish wrap --generation`: *"4 is the default, and that is a measurement rather than a habit.
  Every container found inside real packages for the current console carries the previous
  generation's magic - thirty-three of them, including a working homebrew store."*

## What is wrong with both

**The data table's citation is a fake container.** `PPSA02664` measures `ptype 0x1` in the
32-container census of sweep `20260909-175052` - the fake marker. So the row confirms that a
*fake* eboot carries the previous generation's magic, which is unsurprising: the writer this
table was derived from emits exactly that. It is the same circularity D092 was reversed for,
sitting in a row rather than in a verdict.

**The same probe's census points the other way.** Forty installed eboot containers:
**38 with the current magic, 2 with the previous one.** Whatever the population says, it does not
say what the row claimed.

**And the CLI's thirty-three were counted inside packages.** A package is what this collection
and the homebrew store build; containers found inside them are of that lineage. "Including a
working homebrew store" was offered as strengthening the claim and is the tell that it was the
wrong population - it is more homebrew, not less.

## Why the default stays 4

Because the evidence that actually matters was never the population. **A package this repository
builds with the previous generation's magic installs, mounts, loads and executes on current
hardware** (worklog 040). That is a measurement of the thing the default is for - whether the
file is accepted - and it is untouched by any of the above.

So the default is unchanged and better supported than it was, because "it runs" is a stronger
reason than a count taken from the wrong shelf. Both notes now say that instead.

## What is not being decided

Whether a genuine current-generation eboot carries the current magic. The census suggests it,
38 to 2, but the census does not report magic *per container keyed by ptype*, so it cannot
separate genuine from fake - the same confound recorded against the five header rows. Filed as a
request rather than concluded here. Principle 5: `current_magic` and `previous_magic` keep their
values, and the claim about which file carries which is a row of prose that has been marked
under review rather than replaced by a better guess.

## The general point, which is the fourth of its kind today

The citation was `obscene 048-selfaudit, PPSA02664` - a real probe, a real sweep, a real
container id. Everything about it looked like evidence, and the one thing nobody checked was
*what that container is*. Reading a name as a fact, again: a candidate label, a verdict headline,
a field name, a doc comment, and now a title id in a citation.

---

## Settled, 2026-09-09: refuted, and the default still does not move

REQ-20260909T2020Z-8a17 asked for the magic keyed by `ptype`. The 2x2 across all 32 containers
(sweep `20260909-184538`):

|  | current magic | previous magic |
|---|---|---|
| `ptype 0x0` (genuine) | **23** | **0** |
| `ptype 0x1` (fake) | 7 | 2 |

**The withdrawn claim is refuted rather than merely unsupported.** Every genuine container
carries the *current* magic; none carries the previous one. And its evidence, `PPSA02664`, is one
of the two containers on the box carrying the previous magic - both of which are `ptype 0x1`.

**Here the confound really is broken**, which is worth stating because the five header rows are
still stuck behind theirs. The bottom row is the control group: the fake population spans both
values, so carrying the current magic is not something the console imposes on whatever is
installed. Genuine 23/0 against fake 7/2 is a correlation that survives having a comparison.

## And the default stays 4, which is the same discipline as the entry above

The entry above removed a population claim from the justification. Accepting a different
population claim now would be that error in reverse.

Nothing measured says whether a file built with the *current* magic would be **accepted**. What
is measured is that a package built with the previous one installs, mounts, loads and executes
(worklog 040). Acceptance is the property the default exists to secure; what other people's files
contain is not, in either direction.

That 7 fakes carry one value and 2 carry the other says the console tolerates both from a fake
container - which is consistent with the default being a free choice and is not evidence that it
is. The thing that would move it is a file built here with the current magic, run.

---

## Settled again, 2026-09-09: both magics are accepted, and the default becomes a route decision

REQ-20260909T2125Z-3f9d asked whether obSCEne had already run a container built here with the
**current** magic. They had, twice over:

- `build/eboot.bin` begins `54 14 f5 ee`, built by `mkself --generation 5 --privilege root`,
  installed to `PPSA99980` and launched - **292 checks ran to completion**;
- and the same sweep's payload leg read it back **off console storage**:
  `/system_ex/app/PPSA99980/eboot.bin`, `magic 0xeef51454`, `ptype 0x1`.

So the 2x2 in the section above already contained this repository's own output as one of its
seven fake/current-magic entries, and nobody knew.

## What that changes, and what it does not

**Both values are accepted.** The reason the default stayed 4 up to now - that it was the only
value shown to be accepted - is gone.

**The default stays 4 anyway, on a different reason: it is a route decision.** The two proofs are
on different delivery routes and neither is on the other's:

| route | magic | evidence |
|---|---|---|
| package | previous | installs, mounts, loads, executes (worklog 040) |
| native title directory | current | installs, launches, 292 checks (sweep 20260909-184538) |

`wrap`'s default serves the package path. The native path passes `--generation 5` explicitly and
never consults it. Moving the default would put the package path onto a value nothing has
accepted *on that path*, and D082 already records that the two are different delivery routes
rather than two spellings of one.

## The line that survives all three revisions

The default has never moved on a population, in either direction. Not when the population was
claimed to say *previous* - that claim was withdrawn, and its evidence turned out to be a fake
container. Not when the population turned out to say *current*, 23 of 23 with a control group,
which is a far better measurement than the one withdrawn.

**A default turns on what a loader accepts, and the answer differs by route.** Three revisions of
this entry in one day, and that sentence is the only part that did not need rewriting.
