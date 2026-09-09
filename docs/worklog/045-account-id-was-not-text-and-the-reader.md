# 2026-09-09 - `ACCOUNT_ID` was not text, and the reader refused the file rather than the key


Prosperous asked for an SFO parameter reader it could depend on, so it could stop carrying a
private `\0PSF` parser for one field (REQ-20260909T1244Z-f7ad). The header, the index entries and
the three tables have been here since D059. Reading it should have been a pointer to
`Sfo::parse`, and instead it found two defects.

## The surprise

`ACCOUNT_ID` is eight bytes of user id in the **unterminated** format, `0x0004` - the same
format PS3 saves put text in. `value_of` ran everything in that format through `from_utf8`, so
an id that did not decode returned `SfoError::NotUtf8` and **failed the entire file**. Not the
key. The file. A caller wanting `TITLE_ID` out of a save with an id in it got nothing at all.

`Value::Unknown` already carried the argument against exactly this, in its own doc comment -
"kept rather than refused because the alternative is a reader that fails on a whole file over one
key it did not need". The unterminated format never got the same treatment for one reason: the
only material that had ever reached it was PS3 text, which decodes.

And a second one underneath, which would have been the harder bug to find. Trailing zeroes were
trimmed. That is right for `utf8`, whose stated length includes the terminator, and wrong for a
format with no terminator, where the length is exact and a zero at the end is a byte of the
value. An id ending `0x00` came back seven bytes long and would have been written back a byte
lighter - the same shape as D019 and D020, and findable the same way, byte for byte against real
material.

## What was built

- `Value::Binary(Vec<u8>)` beside `TextUnterminated`. Both write `utf8_special`, so D020's rule
  is untouched: the variant still determines the format code. Nothing is trimmed in either.
- `Value::as_bytes` and `Sfo::bytes(key)`, which answer for every kind but an integer.
- `crates/selfish-title/examples/sfo_params.rs` - every parameter with its format, its variant
  and its byte count, and named keys dumped as hex. Run against a generated file it reads
  `ACCOUNT_ID = ff01009a2b007cd3 (8 bytes, as written)` beside `TITLE_ID` as text.
- Five tests, including the one that matters most below.

## The trap that nearly shipped inside the fix

The obvious API is "match on `Value::Binary`". It is wrong, and quietly.

**Some eight-byte ids decode as UTF-8 by luck**, and those come back as `TextUnterminated`. A
consumer matching on `Binary` alone would work on most saves and miss on a few - and the symptom
would be a bad save rather than a bad reader, which is worse than never working. So the accessor
is not the variant: `bytes(key)` answers whichever variant the parse chose, and
`bytes_answers_for_a_value_that_decoded_as_text_too` pins it with an id spelled `abcdefgh`.

Left undecided on purpose: **which end of a user id is significant**. `bytes` returns file order.
Rendering eight bytes as a number is choosing an endianness, and this repository has already been
caught by that choice once, on a hash where the wrong end gave a plausible value that resolved
nothing. The bytes are the fact; the number is the consumer's interpretation. (D090)

## The general shape of it

D062 recorded that a crate missing from `CLAUDE.md` gets written a second time. This is the
adjacent failure and worth naming separately: **the crate was here, listed, and complete about
the layout - and a second parser grew anyway, at the accessor.** Prosperous needed raw bytes for
one key; the crate had a reader, a writer, a provenance table and 23 tests, and no way to ask
that question. Completeness about a format is not the same as being usable for what a consumer
holds, and the second copy appears wherever the gap is.
