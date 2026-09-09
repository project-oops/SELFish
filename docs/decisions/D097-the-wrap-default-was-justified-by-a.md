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
