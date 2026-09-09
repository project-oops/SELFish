# D092 - The container header is confirmed at the current generation, and the limit it replaces is categorical rather than generational

**Status: decided. 2026-09-09.**


**A real current-generation retail vendor game confirms all nine fixed `self_header` rows. The
limit that replaces the old one is not "which generation" but "which kind of container".**

`data/self-format.tsv` was derived entirely from previous-generation material - a PS4 toolchain
and two PS4-generation readers - and said so in bold: *nothing here is confirmed for the current
generation*. D084 built `selfish audit` to check a real container against it, D085 tried to get
one and was on the wrong side of the provenance line, and D086 replaced the dump with an
on-console comparison that carries only the verdict off the box.

That chain now has its answer. obSCEne's `048-selfaudit`, sweep `20260909-144348`, read
`/system_ex/app/PPSA03416/eboot.bin` - a retail vendor game at the current generation - and
**all nine fixed rows matched**: `version` 0x0, `mode` 0x1, `endian` 0x1, `attributes` 0x12,
`key_type` 0x101, `category` 0x1, `program_type` 0x1, `flags` 0x22, `padding3` 0x0.

Principle 2 usually shows up as a refutation - D019 is the source that eleven real files
overturned. This is the same principle running the other way, and it is the rarer result:
derived from citable sources that describe a *different console*, and confirmed unchanged by
the one it was aimed at.

## What the divergence turned out to be

The sweep before it audited only `/system/vsh/app/NPXS40038/eboot.bin`, a vendor **system app**,
where five of the nine differ - `version` 0x10, `attributes` 0x32, `key_type` 0x10000301,
`program_type` 0x3, `flags` 0x52 - and reported `diverged`, with the gloss that the table's
current-generation rows needed review. Read alone, that says the rows are wrong.

Read beside the game, it says something else: **these five are not invariants of the format.**
They are pinned for an *application* container, and a system container is a different kind of
file. `program_type` announces that in its own name. `flags` was never a constant either - the
table's own note derives it as `0x2 | (signed_block_count & 0x7) << 4`, and `0x52` is that same
formula at five signed blocks where `0x22` is two. A row that reads like a constant is a
default.

The other three - `version` 0x10, `attributes` 0x32, and the `0x1000` in the high half of
`key_type` - are **recorded as observed and not explained**. What they mean at a system tier
needs a citable source, and principle 5 is explicit that an unexplained value is named and left
alone: a plausible guess in a container produces a file that is accepted and then read from the
wrong place.

So the header keeps a limit, and it is a better one. Not "this is the previous console's
container" but "these five rows describe an application container, and the format does not
promise them of anything else".

## The part that does not fit, recorded rather than smoothed over

The sweep also read `/system_ex/app/FAKE00000/eboot.bin`, which the probe classed as **fake**,
and it carries the system app's five values exactly. The resolution to REQ-20260909T1450Z-3c7e
reads that as a container this collection built, and concludes the split is between application
containers and "signed system apps / root apps".

**That cannot be right about anything built here, and the code is unambiguous.** `Privilege` in
`selfish-container` is used in exactly one place - `constants.paid = privilege.paid(constants.paid)`
- and `paid` lives in `self_ex_info`. The five header fields come from this table for every
tier, so `mkself --privilege root` writes `program_type` 0x1 and `flags` 0x22 like everything
else. A container built here would have *matched*, the way the game did.

Which leaves two possibilities, and this repository cannot choose between them from here:
`FAKE00000` was not built by this collection, or something rewrote its header afterwards. The
probe's `fake` label classifies the file - a container declares itself fake in a field the
format provides - and is not a record of what produced it.

The confirmation does not depend on which it is, so it is applied now and the discrepancy is
filed as its own question (a new request, because a resolved one is never reopened). The reason
to write it down rather than let it pass: **an attribution that happens to be convenient is the
kind this collection has been caught by before**, and "our own builder emits system values" is a
claim that would have quietly justified changing `mkself`.

## Closed, 2026-09-09: it was not ours

The question above was filed as REQ-20260909T1530Z-7e11 and answered the same hour.
`/system_ex/app/FAKE00000/eboot.bin` was **not built by this collection**. The path is a
hardcoded candidate in obSCEne's probe - an entry in `obs_audit_candidates[]` in
`src/probe/sections/selfaudit.c`, a place to *look* for a pre-existing third-party container -
and the `fake` beside it labels what the probe hoped to find, not what produced the file. The
"built by collection" attribution was an assumption drawn from that label, and it reached two
resolutions before anyone checked it.

So `mkself` is sound and this section's argument stands as written. Two things worth keeping
from how it went:

**A label in a candidate list is not provenance.** `{"fake", "/system_ex/app/FAKE00000/..."}`
is a hope about a path. Two hops downstream it was being reported as a fact about a builder,
and the fact would have justified changing what that builder writes.

**The file is more useful than the puzzle was.** Something that is demonstrably not this
repository wrote a container agreeing with a vendor system app on all five non-application
values. That makes it a *second, independent producer* of those values rather than a loose end -
weak evidence, because nothing here knows what wrote it, but evidence pointing the same way as
the vendor container rather than at us.
