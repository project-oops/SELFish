# D091 - A writer must refuse where a reader may guess, and this crate had them the wrong way round

**Status: decided. 2026-09-09.**


**A writer must refuse where a reader may guess. `selfish-elf` had two `string_at`
functions - the reader's strict, the builder's lossy - and that is the wrong way round.**

D090 had just finished arguing that a *reader* refusing a whole file over one field it was
never asked for is a defect. The obvious next question is where else this crate decides how
permissive to be, and the answer was one crate away: `selfish-elf` reads names from a string
table in two places, and the two disagree on all three edge cases.

| the offset is | `dynamic::string_at` (reader) | `dynlib::string_at` (builder) |
|---|---|---|
| past the end of the table | `Err(StringOutOfRange)` | `""` |
| unterminated | `Err(UnterminatedString)` | the rest of the table, as one name |
| not UTF-8 | `Err(StringNotUtf8)` | lossy - `U+FFFD` |

The reader can only report what somebody else wrote, so refusing there is at worst unhelpful.
The builder's result is **written back into the module**, and there every one of those three
answers is a name this crate invented.

## What each one actually did, measured rather than argued

**Lossy conversion changed a symbol's name.** A defined symbol named `6c 6f ff 63` came out of
the rebuilt string table as `6c 6f ef bf bd 63`: a different name, two bytes longer, in the
table a loader resolves against.

**An out-of-range offset defeated `Unclaimed`, which is the worse one.** `rebuild_symbols`
decides a symbol is an import with `read_u16(entry, 6)? == 0 && !plain.is_empty()`. An
out-of-range offset produced `""`, so the second half was false, so an *undefined* symbol was
treated as locally defined: it never reached `resolve`, was never reported unclaimed, and was
written back as a nameless local. `build` returned `Ok` with `encoded == 0`. The module would
build, load, and jump to a slot nothing filled in - which is exactly the failure `Unclaimed`
exists to make impossible, arriving down the one path that skipped the check.

That path is reachable rather than theoretical. `Linked::names` is supplied by the caller, and
this crate has two string tables to confuse: hand it `.strtab` where `.dynstr` was wanted and
every offset is out of range at once. The answer to that mistake was a module full of nameless
symbols, built successfully.

## The rule

`dynlib::string_at` now returns `Result<String, BuildError>` and refuses all three cases, with
one new variant, `BuildError::SymbolName(u32)`, carrying the offset - the offset because the
usual cause is a wrong table rather than a wrong byte, and an offset says which.

**The reader is deliberately left alone.** Its permissiveness is a separate question with a
separate answer, and D090's lesson does not transfer by symmetry: a name that a real module
carries and this crate cannot decode would be an argument for loosening it, and no such module
has been seen. Principle 5 - the change to make is the one material asks for, not the one that
looks consistent.

Every existing test passed unchanged against the stricter builder, which is the useful negative
result: nothing real was relying on any of the three.

## Why this is worth a numbered entry rather than a fix

Because the two functions had the same name, in the same crate, for the same job, and were
written to opposite standards without anybody choosing that. The generalisation is what to
carry forward: **which side of a format library you are on decides how permissive to be, and
"lossy" is a reasonable default on exactly one of them.** A reader that guesses produces a
wrong answer on a terminal. A writer that guesses produces a wrong file, and a wrong file is
what this repository exists to stop.
