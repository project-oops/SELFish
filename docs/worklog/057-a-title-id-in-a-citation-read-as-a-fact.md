# 2026-09-09 - A title id in a citation, read as a fact


Quiet tick, nothing outstanding, so: the suffix file had carried a false claim about a sibling
repository for however long, and the check that found it was reading twenty lines of theirs. Are
there others? Bounded version - grep the `data/` files, which are the source of truth, for
claims about other projects.

Nine hits. Seven are provenance citations or this session's own measurements. Two are the
generation-magic rows, and they are wrong in a way that took ten minutes to establish and would
never have been noticed by reading them.

## The claim

`data/self-format.tsv`, `current_magic`: *"a current-generation title's eboot carries
previous_magic (4F 15 3D 1D), measured on hardware (obscene 048-selfaudit, PPSA02664: gen4
eboot=1, gen5=0)"*. And `previous_magic`: *"what a current-generation title's EBOOT actually
carries (hardware-confirmed)"*.

**`PPSA02664` measures `ptype 0x1` in today's census. It is a fake container.** So the citation
establishes that a *fake* eboot carries the previous generation's magic - which is what the
OpenOrbis-lineage writer this table came from emits. The same circularity D092 was reversed for,
this time living in a row rather than in a verdict.

And the same probe's census counts, across forty installed eboots, **38 with the current magic
and 2 with the previous one.** The population says the opposite of what the row claims.

## What rested on it

`selfish wrap --generation` defaults to 4, and its help said: *"4 is the default, and that is a
measurement rather than a habit. Every container found inside real packages for the current
console carries the previous generation's magic - thirty-three of them, including a working
homebrew store."*

Thirty-three containers **found inside packages**. Packages are what this collection and the
homebrew store build, so containers inside them are of that lineage. "Including a working
homebrew store" was written to strengthen the claim and is the tell that the population was
wrong - it is more homebrew, not less.

## The default does not move

Because the evidence that matters was never the count. A package this repository builds with the
previous generation's magic **installs, mounts, loads and executes on current hardware**
(worklog 040). That measures the thing the default is for - whether the file is accepted - and
nothing above touches it.

So both notes now rest on that instead, and the default is better supported than it was. "It
runs" beats a census taken from the wrong shelf. What is *not* decided is whether a genuine
current-generation eboot carries the current magic: the census does not report magic per
container keyed by `ptype`, so it cannot separate genuine from fake - the same confound already
recorded against the five header rows. That goes out as a request. (D097)

## Fifth

A candidate label, a verdict headline, a field name, a doc comment, and now a title id inside a
citation. `obscene 048-selfaudit, PPSA02664` looks exactly like evidence: real probe, real sweep,
real container. The one thing unchecked was **what that container is** - and this repository had
spent the afternoon building a tool that answers precisely that question and printing it above
the row count so nobody could miss it.

It was missed anyway, in a file the tool does not read.
