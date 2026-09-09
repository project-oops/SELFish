# 2026-09-09 - The magic, settled by the row nobody asked for


Worklog 057 marked the `current_magic` / `previous_magic` rows under review and filed
REQ-20260909T2020Z-8a17 for the magic keyed by `ptype`. It came back, and it is the most decisive
measurement of the day:

```
                     current magic    previous magic
  ptype 0x0 (genuine)      23                0
  ptype 0x1 (fake)          7                2
```

Every genuine container carries the current magic; none carries the previous one. And the claim
withdrawn earlier that afternoon - that a current-generation title's eboot carries the *previous*
magic, marked hardware-confirmed - is now **refuted** rather than merely unsupported: its
evidence was `PPSA02664`, which is one of the two previous-magic containers, and both are fake.

## What made it settle, which is the part worth carrying

**The bottom row.** It was not asked for - the request wanted the magic beside `ptype`, and the
2x2 fell out of that - and it is the whole reason this question is closed while the five header
rows are not.

Genuine 23/0 on its own proves very little. "Every container on a current-generation console
carries the current magic" would explain it without reference to authenticity at all, and this
repository has spent the day being caught by exactly that class of alternative. The fake row is
what removes it: 7 carry one value and 2 carry the other, so the console does **not** impose the
current magic on whatever is installed. The correlation survives having something to compare
against.

That is precisely what the five header rows lack. Every `ptype 0x1` container there is
`PPSA`-shaped and every `ptype 0x0` one is `NPXS`-shaped, with no case on the other diagonal - so
there is no control and the confound stands. **The difference between a settled row and a stuck
one, on this console, is whether the sample happened to contain a counterexample class.**

## The default did not move, and that is the same discipline twice

D097 removed a population claim from the `wrap` default's justification in the morning. Accepting
a different population claim in the evening - "genuine containers carry the current magic, so
write that" - would have been the identical error reversed, and it was tempting precisely because
the new measurement is so much better than the old one.

The default turns on **acceptance**, not on population. What is measured is that a package built
with the previous magic installs, mounts, loads and executes (worklog 040). Nothing measures
whether a file built here with the *current* magic is accepted, so the default stays where the
evidence is.

Filed as REQ-20260909T2125Z-3f9d - and that request is mostly a retraction of a guess. obSCEne's
`Makefile` defaults `EBOOT_GEN` to 5 on the Prospero path and passes it to `mkself --generation`,
and their eboot leg runs, which looks like the measurement already exists. It was asked rather
than concluded: a Makefile default, a fix described in a comment, and a log line reading
`generation|known|5 (agc)` that is the probe detecting the *console* rather than reporting its own
container, is the same three-input inference that has cost five corrections today.

## One protocol note

obSCEne re-filed under `REQ-20260909T1715Z-5b91`, an id already resolved in this inbox, so that
id now appears twice - once resolved, once open. The resolved block was not touched and the new
content was serviced as a new ask, but commit messages here cite request ids, and an id that
resolves to two blocks cannot be cited. Flagged back to them.
