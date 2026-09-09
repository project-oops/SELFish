# D094 - The audit checks the tail it pins, and reports it apart from the header

**Status: decided. 2026-09-09.**


**`selfish audit` now checks the four `ex_info` rows the table pins, and reports them as their
own block rather than folded into the header count.**

`data/self-format.tsv` pins the whole container, but the audit only ever checked `self_header`.
The tail - `[ex_info 64][npdrm 48]`, the last `0x70` bytes before `header_size` - is the part
the charter is most cautious about: it came from OpenOrbis, a **PS4 writer**, and is the only
section no reader could have supplied, because a reader walks a container to find an ELF and
never looks at it.

So the least-supported rows in the table were the ones the oracle command could not check. That
became concrete rather than theoretical when a hardware sweep reported `ex_info | not located`
on nine containers and this repository had no way to try the same read itself.

## Reported apart from the header, and that is the decision

The obvious implementation adds the tail rows to `header` and lets `confirmed()` count them. It
is wrong, because the two blocks answer different questions:

- a **header** row differing is a claim about the container format - the thing D092 spent a
  reversal on;
- a **tail** row differing usually just means *this is not a fake container*. The values came
  from a writer that made no other kind: `ptype` `0x1` is the fake marker, `paid`
  `0x3100000000000002` is that writer's default. Any vendor container differs here by
  construction.

One number covering both would mean neither, and it would manufacture exactly the false
divergence that made a system app look like a refutation of the format. `tail` and
`tail_differing()` sit beside `header` and `differing()`, and the CLI prints the tail under its
own heading with a line saying why a difference there is expected.

## Two things it does not do

**It does not read the digest.** Thirty-two bytes of SHA-256 is not a field an audit can confirm
against a table - the table pins no value for it - and dumping it would be the shape D086 ruled
out.

**`tail` and `declared` are computed from one function.** `ex_info_at` is the only place the
`header_size - 0x70` arithmetic is written. It was briefly written twice, once for the `ptype`
read and once for the rows, and two copies of a layout offset is precisely the defect the whole
`data/` discipline exists to prevent. If the tail is unreachable, both come back empty together:
an audit that could report four confirmed rows while saying it never located the block would be
worse than one that reports nothing.
