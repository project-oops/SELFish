# 2026-09-09 - The same function twice, written to opposite standards


D090 finished by arguing that a reader refusing a whole file over one field is a defect. The
next question was where else this crate decides how permissive to be, and the answer was one
crate away and pointing the other way.

`selfish-elf` has two functions called `string_at`. `dynamic::string_at` reads a name out of a
module somebody else wrote and returns `Err` on an out-of-range offset, an unterminated string
or invalid UTF-8. `dynlib::string_at` reads a name that is about to be **written back** and
answered all three with a guess: `""`, the rest of the table, and a lossy `U+FFFD`.

Same name, same crate, same job, opposite standards, and nobody chose it.

## What they did, measured

Two throwaway tests, run before anything was changed, because the existing harness takes `&str`
and so cannot express either case:

- A defined symbol named `6c 6f ff 63` came back out of the rebuilt string table as
  `6c 6f ef bf bd 63`. A different symbol name, two bytes longer, in the table a loader
  resolves against.
- An undefined symbol whose name offset pointed past the end built **successfully**, `Ok` with
  `encoded == 0`.

The second is the one worth remembering. `rebuild_symbols` decides a symbol is an import with
`read_u16(entry, 6)? == 0 && !plain.is_empty()`. An out-of-range offset gives `""`, so the
second half is false, so an undefined symbol looks locally defined - it never reaches `resolve`,
is never reported `Unclaimed`, and is written back nameless. The module builds, loads, and jumps
to a slot nothing filled in. `Unclaimed` exists precisely to make that impossible, and this went
round it.

And it is reachable rather than theoretical: `Linked::names` comes from the caller, and this
crate has two string tables to confuse. Pass `.strtab` where `.dynstr` was wanted and every
offset is out of range at once - to which the honest answer is an error, and the answer given
was a module full of nameless symbols.

## What changed

`dynlib::string_at` returns `Result<String, BuildError>` and refuses all three, through one new
variant `BuildError::SymbolName(u32)`. The offset is carried because the usual cause is a wrong
*table*, not a wrong byte, and an offset says which.

**Every existing test passed unchanged**, which is the useful negative result: nothing real was
relying on any of the three guesses. 246 pass now, three of them new and each named for the case
it protects.

The reader was deliberately not touched. D090's lesson does not transfer by symmetry - loosening
it would need a real module carrying a name this crate cannot decode, and there isn't one.
Principle 5: make the change material asks for, not the one that looks consistent. (D091)

## The thing worth carrying

**Which side of a format library you are on decides how permissive to be, and "lossy" is a
reasonable default on exactly one of them.** A reader that guesses puts a wrong line on a
terminal. A writer that guesses puts a wrong file on a console, and that is the failure this
repository exists to prevent. Two consecutive days produced one defect in each direction, which
suggests the question is worth asking of every place this collection converts bytes to a
`String`.
