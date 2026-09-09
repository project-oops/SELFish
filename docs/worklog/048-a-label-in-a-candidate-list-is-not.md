# 2026-09-09 - A label in a candidate list is not provenance


Yesterday's entry ended with a container that did not fit: `/system_ex/app/FAKE00000/eboot.bin`,
reported in two sweep resolutions as a container **built by this collection**, carrying the five
header values a vendor system app carries rather than the ones `mkself` writes. It was filed as
a question rather than acted on, because the tidy reading - "our own builder emits system
values" - would have justified changing a builder whose output installs and executes.

Answered within the hour, and the answer is that it was never ours.

The path is a **hardcoded candidate** in obSCEne's probe: an entry in `obs_audit_candidates[]`
in `src/probe/sections/selfaudit.c`, put there to check whether a pre-existing third-party
container happened to be installed on the test console. The `"fake"` beside it labels *what the
probe hoped to find at that path*. Nothing in any repository builds a `FAKE00000`.

So the label was a hope about a path, and two hops downstream it was being reported as a fact
about a builder.

## Why it was worth a request rather than a shrug

Three checks, all cheap, all in this repository, and any one of them would have stopped the
story:

- `Privilege` is used in exactly one place - `constants.paid = privilege.paid(constants.paid)` -
  and `paid` lives in `self_ex_info`, not the header.
- `write_header` takes `generation` and spends it on `out.raw(&generation.container_magic())`:
  four bytes, nothing else.
- Between them those are the only two knobs obSCEne turns, so a root-tier gen-5 container built
  here carries the *application* values and would have audited `confirmed`, exactly as the
  retail game did.

The attribution was convenient - it closed the loop and explained the divergence in one move -
and convenience is precisely when a claim gets the least scrutiny. **An attribution that happens
to be tidy deserves the same check as one that is not.**

## The consolation, which is better than the puzzle

Something demonstrably *not* this repository wrote a container that agrees with a vendor system
app on all five non-application values. That makes `FAKE00000` a **second, independent
producer** of those values rather than a loose end. Weak evidence - nothing here knows what
wrote it - but it points at the vendor container rather than at us, which is the direction that
matters.

The five rows stay recorded as observed and unexplained. Principle 5 has not moved: what a
system container's `version` 0x10 or `attributes` 0x32 *mean* still needs a citable source, and
two producers agreeing is not a derivation. (D092)

## One more thing the exchange bought

The earlier ambiguity - two `selfaudit/source` lines with one block of values between them - had
a cause worth knowing about, and obSCEne found it: their `.obs.log` extraction runs
`awk '!seen[$0]++'`, which deduplicates *identical field lines across containers*. Nine
containers, most measuring identically, collapsed into one readable block and eight bare source
lines. The measurement was never ambiguous; the extraction made it so. Refusing to read the
summary and asking for the attribution was the right call, and it is a reminder that a
deduplicated log is a lossy one when the same fact is true of several things.
