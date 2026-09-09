# D093 - The audit reports what kind of container it just agreed with, before it reports how many rows agreed

**Status: decided. 2026-09-09.**


**`selfish audit` now says what kind of container it read, above the row count, because the
count is meaningless without it and the count is what a reader remembers.**

D092 recorded a confirmation that had to be reversed: a container matched all nine fixed header
rows, and the container was fake. A file written from this table agrees with this table, so the
match was a round trip. The value that would have said so - `ex_info.ptype` - was in the same
log, thirty lines away, and nobody looked because the number at the top already read as an
answer.

The tool made that easy. `audit` returned a generation and a list of row verdicts, and the CLI
printed `confirmed 9 of 9`. Nothing in either said which of the two things a reader was holding.

Worse, the crate **already knew**. `a_container_this_crate_builds_confirms_every_fixed_row_of_the_table`
has been in the test suite since D084, and its own comment calls the thing it asserts a round
trip. The knowledge was written down in a place that runs on every commit and never reaches a
person reading output.

## What changed

`Audit` carries a `declared: Declared`, read at the offset the table pins for `ex_info.ptype`:

- `Declared::Ptype { value, known }` - the raw value, and the name the table's `ptype` group
  gives it, if any.
- `Declared::Unreachable` - the tail is not at `header_size - 0x70`, or the file stops first.

`Declared::is_round_trip()` is true only for `fake`. `Declared::caveat()` returns the sentence
to print. The CLI prints both **above** the row count, because a caveat under a headline is a
footnote and a footnote is what failed here.

## Reported, not tested - and this is the whole of it

The obvious implementation is to decide "did I find `ex_info`" by checking whether the value is
a `ptype` the table names, and report *not located* otherwise. That is what obSCEne's probe
does, and it is why nine containers in one sweep came back `ex_info | not located`: the table
names fake, npdrm, system and secure types from a PS4 writer, so an unrecognised value is at
least as likely to mean *this table does not know this kind* as *the tail is elsewhere*. Folding
those two into one verdict destroys the distinction and reads as a fact about the format rather
than about the reader.

So the value is returned whatever it is, and `known: None` is a separate, quieter statement. A
caller can see `ptype 0x1234, not a kind this table names` and decide for itself; it cannot see
that through a boolean.

`is_round_trip()` is deliberately `false` for an unknown value. **Not known to be fake is not
known to be genuine**, and a helper that returned "genuine" for anything it failed to recognise
would rebuild the original mistake behind a nicer name. The caveat says so in words instead.

## What it does not do

It does not refuse, warn on stderr, or change an exit code. An audit of a fake container is a
completely legitimate thing to run - it is principle 4's round trip, and the test above depends
on it. The defect was never that the round trip happened; it was that the output did not say
which one it was. A tool that made this an error would be answering a different complaint.
