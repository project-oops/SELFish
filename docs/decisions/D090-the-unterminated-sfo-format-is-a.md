# D090 - The unterminated SFO format is a length, not a promise of text, and reading it as text failed whole files

**Status: decided. 2026-09-09.**


**The unterminated SFO format is a length and nothing else. Treating it as text refused whole
files over one key, and trimming it lost a byte of a value that had no padding to lose.**

`param.sfo` has three value formats. Two are unambiguous: `utf8` states a length that *includes*
its terminator, and `integer` is four little-endian bytes. The third, `utf8_special` (`0x0004`),
is a length with no terminator - and this crate read its name as a description of its contents.

That was true of the only material that had reached it. PS3 saves put unterminated text there,
D020 was written about exactly that, and eleven real files round-tripped byte for byte. What
none of them contained was `ACCOUNT_ID`: eight bytes of user id, in that same format, which a
current-generation save carries and which is not text at all.

Two failures followed from one assumption, and both were silent until somebody asked:

- **`from_utf8` returned `Err`, so `Sfo::parse` returned `NotUtf8` and the whole file failed.**
  A reader that refuses a file over a key the caller never asked for is the failure
  `Value::Unknown` already exists to prevent - its own documentation says so - and the only
  reason this format was not treated the same way is that the first bytes to arrive happened to
  decode.
- **Trailing zeroes were trimmed.** Correct for `utf8`, where the terminator is inside the
  stated length; wrong here, where the length is exact and a zero at the end is a byte of the
  value. An id ending in `0x00` came back seven bytes long and was written back a byte lighter,
  which is the same class of defect D019 and D020 were both about and would have been found the
  same way - byte for byte, against real material.

## What replaces it

`Value::Binary(Vec<u8>)`, beside `TextUnterminated`. Both write `utf8_special`, so **D020's rule
holds unchanged**: the variant still determines the format code, and a value still cannot be held
alongside a format it disagrees with. What a parse chooses between them depends only on whether
the bytes decode, and nothing is trimmed either way.

That leaves one trap, and closing it is the reason for the second half of this entry. **Some
eight-byte ids decode as UTF-8 by luck.** A consumer matching on `Value::Binary` would work on
most saves and silently miss on the rest - a failure that presents as a bad save rather than a
bad reader, which is worse than never working at all.

So the accessor a caller reaches for is not the variant. `Value::as_bytes` and `Sfo::bytes(key)`
answer for **every** kind but an integer, returning the value's bytes as the file holds them,
without the terminator that belongs to the format. `sfo.bytes("ACCOUNT_ID")` is correct whichever
variant the parse produced, and a caller never has to know which it got.

## What this crate deliberately does not decide

**Which end of a user id is significant.** `bytes` returns them in file order and stops there.
Rendering eight bytes as a number requires choosing an endianness, and this repository has been
caught by that choice once already, on a hash where the wrong end produced a plausible value that
resolved nothing (D004). A format library that guessed here would be making a consumer's decision
in a place the consumer would never think to look. The bytes are the fact; the number is an
interpretation, and it belongs where the comparison happens.

## Why it lives here rather than in the project that asked

Prosperous filed REQ-20260909T1244Z-f7ad because it carries a private `\0PSF` parser to read
`ACCOUNT_ID` for save retargeting, and its own principle 6 says it reads the platform's formats
from here and invents none. D059 already settled that a `PARAM.SFO` is a format and belongs in
this repository, and D062 records what happens when a crate is not visible from where somebody
is standing: a second implementation gets written, hardcoding offsets the first one reads from
`data/sfo-format.tsv`.

The header, the index entries and the three tables were all here already. What was missing was
the one accessor that made the crate usable for the thing a consumer actually needed - and its
absence is what a second parser had grown around. That is worth noting on its own: **a format
crate can be complete about the layout and still unusable, and the second copy appears at the
accessor rather than at the table.**
