# D098 - Measured vendor values are recorded against the rows they refute, never as a profile a writer could reach for

**Status: decided. 2026-09-09.**


**A measured vendor constant is an observation about a row. It is not a second specification, and
the difference is the difference between recording what a file was and offering to build one.**

obSCEne asked for the nine fixed header values measured across 23 genuine containers to be
recorded in `data/self-format.tsv` as a **"signed vendor profile"**, set beside the existing rows
as *"an unambiguous specification of gen-5 signed vendor containers versus fake containers"*
(REQ-20260909T1715Z-5b91). The measurement is sound and most of it was already recorded. The
framing was declined, and the reasons are worth a number because the request will be made again -
by a consumer, or by this repository in six months.

## Why not a second profile

**1. A profile with a value column is a thing a writer can emit.** That is the whole of it.
`data/` is the source of truth and code reads it rather than carrying its own copy - which is the
point of the discipline and, here, the hazard. A `self_header` row set labelled *signed vendor*,
carrying values, is directly reachable by `mkself` and by anything else reading the table. A
container built from it would carry a header asserting it is a signed vendor executable.

Principle 6's line is **never claim to be the vendor**. D047 marks where that line actually sits:
a package's licence is signed with the **debug** RIF keyset, and that is fine because signing
with a debug key asserts *"this is a debug licence"*, which is true, and the licence says so in
its own type field. A header claiming the vendor profile asserts something that is not true, and
no field in it says otherwise.

Recording the same numbers as notes on the rows they refute cannot be built from. That is not a
technicality - it is the entire difference, and it is why the same bytes are acceptable in one
column and not another.

**2. The sample cannot support the name.** Every `ptype 0x1` container in the census is
`PPSA`-shaped and every `ptype 0x0` container is `NPXS`-shaped: authenticity and container kind
are perfectly confounded, so the five differing rows may describe a *signed vendor* container or
a *system* container. Calling the profile "signed vendor" settles that by naming it.

This repository made exactly that mistake in the opposite direction the same afternoon, calling
the limit categorical - *"these five rows describe an application container"* - and had to
correct it for reading one arm of a confound as the cause. Accepting "signed vendor" would be
the same error, arrived at by agreeing rather than by inventing, which D095 records as costing
just as much.

And `ptype 0x0` is not one of the eight values this table's `ptype` group names. Naming it from
one console's population is what principle 5 exists to stop.

## What is recorded instead

The measured values, on the rows they refute, in the note column: `version [genuine: 0x10 on
23/23, sweep 20260909-175052]` and the same for the other four. Plus the census, the split and
the confound in the header. That is the form principle 2 sanctions - a real file **confirms or
refutes** a structure taken from cited sources, and which rows reality settled is recorded.

The request also produced the finding it did not ask for, which is recorded the same way:
`mode`, `endian`, `category` and `padding3` hold on all 23 genuine containers as well as all 9
fake, and are marked `CONFIRMED` in place. First confirmation this table has had from material
this repository could not have produced.

## Where this was nearly lost

The reasoning above existed only in a resolution written into `C:\tmp\SELFish\worklog.md` - an
inbox that lives outside every repository and is committed nowhere. A ruling about what may enter
`data/` was held in a file that is not in `data/`, not in the repository, and not backed up by
anything.

That is the day's own lesson arriving one more time: a decision recorded only where it was made
is a decision that does not survive. The inbox is where a request is answered; the decision log
is where the answer has to live.
