# D028 - The header identity is stamped here too, and `e_type` is a parameter rather than a constant


Three fields a loader checks before it reads anything else - `EI_OSABI`, `EI_ABIVERSION` and
`e_type` - and no linker sets any of them, because no linker knows about either console. Get
one wrong and the file is refused with a message about the header:

```
IsElfFile: e_ident[EI_OSABI] expected 0x09 is (0x0)
IsElfFile: e_type expected 0xFE10 OR 0xFE18 OR 0xfe00 is (0x3)
```

This was found by running the finished chain end to end and looking at the output: a module
built by this repository's own script, wrapped in this repository's own container, came out as
an ordinary `ET_DYN` with a SysV ABI. Every unit test passed. The reader had been printing
*"osabi NOT FreeBSD - a loader refuses this before anything else"* the whole time.

**`e_type` is a parameter.** An executable and a shared library are both legitimate outputs,
they are different files, and only the builder knows which it is making. obSCEne hardcodes one
because it builds one kind of thing; a shared library has no business inheriting that.

It is also the field with the worst history. Those two constants were named the wrong way round
for months in a sibling project, so its builder wrote "shared library" while its log said
"executable", and the symptom was exactly what the comment sitting above them predicted and
nobody connected: a loader maps the module, runs its initialisers, then looks elsewhere for a
process to start. It loads. It never runs.

`Generation` stays a parameter for the same reason it is everywhere else here: the loaders
disagree about `EI_ABIVERSION` and both are right about their own generation.

Two smaller choices:

- **Idempotent, and it reports what it changed.** A field already correct produces no `Change`,
  so targeting the previous generation reports two changes rather than three - its ABI version
  is zero, which is what a linker already leaves. A tool claiming credit for a byte it did not
  touch is a tool nobody can debug.
- **An unrecognised `e_type` is refused rather than overwritten.** Stamping one onto a
  relocatable object would assert something untrue about a file this code does not understand.

