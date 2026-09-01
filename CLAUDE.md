# CLAUDE.md

How selfish is built and the constraints to honour when changing it.

**Read [the OOPS conventions](https://github.com/project-oops/OOPS/blob/main/docs/CONVENTIONS.md) first.** Provenance, naming, decision logs, worklogs and
gates are shared across [Orbistoun](https://github.com/project-oops/Orbistoun), [obSCEne](https://github.com/project-oops/obSCEne), [Prosperous](https://github.com/project-oops/Prosperous) and
[SELFish](https://github.com/project-oops/SELFish), and are stated once there. This file holds only what SELFish adds.

## Mission, in one breath

Rust libraries and a command-line tool for the file formats Prospero-generation hardware
loads, read and written in one place. ELF as the platform spells it, the signed-executable
container, packages and the filesystem inside them, and the import hash. Nothing else.

## Why it exists, which is a thing that already happened

Three projects need these formats. orbistoun reads them to load a title, obSCEne writes them
to produce one, prosperous inspects them over a wire. Before this repository the knowledge
lived in whichever project happened to need it first, and the cost was not hypothetical:

**obSCEne shipped a container builder that emitted the previous generation's magic.** The
current one - `54 14 F5 EE` - was recorded in orbistoun's decision log, status *observed from
real material*, one directory away. obSCEne's own format table said "nothing here is confirmed
for the current hardware" and treated that as a limit to note rather than a question somebody
had already answered. A file built by it would have been rejected by the target on its first
four bytes.

obSCEne's `CLAUDE.md` already contains the argument for why this repository exists, written
about a directory rather than a repository:

> retyping a judgement into a new language is how a transcription error gets into a census

That reasoning does not stop at a repository boundary.

**The second reason is contention, and it is just as real.** A repository is a unit of
concurrent work. While the formats lived inside obSCEne's `tool/`, work on a package writer
and work on the probe were the same working copy - so they serialised, and one session's
refactor blocked the other's feature. Splitting the formats out removes a duplication *and* a
queue.

## The admission test

Anything here holds **knowledge about a file format, and nothing that knows what a consumer
is for**.

A container builder qualifies. obSCEne's check registry never does, whatever it depends on.
When something is borderline, the question is whether it would still make sense if the three
consuming projects did not exist.

Two rulings made on day one, because both are the thin end of a wedge:

- **The import hash is in. The mined corpus is not.** The hash is a format: one algorithm,
  two implementations before this repo, and a wrong answer is a wrong answer everywhere. The
  corpus of a million-odd identifiers is obSCEne's *mining output* - a measurement product
  that happens to be stored as data. It stays there.
- **Knowledge, not runtime.** selfish holds what you need to *know*; it does not hold what you
  need to *link*.

  This line was first drawn as "not an SDK", which did not survive being questioned. These
  projects **are** a homebrew SDK: obSCEne compiles for the hardware with no vendor toolchain
  anywhere - clang, `lld`, its own linker script, its own platform declarations, and its own
  format tooling. Denying that was guarding a real risk with a slogan.

  The real risk is a repository that grows a runtime. So: file formats, the import hash,
  segment layout rules and linker scripts are knowledge and belong here. A `crt`, an
  allocator, string functions - anything that *executes on the hardware* - do not, and neither
  does a convenience layer for writing homebrew.

  Platform ABI declarations sit on the line and are **not resolved yet**. Several hundred
  function signatures are declared in a probe in order to call them and implemented in an
  emulator in order to provide them: two consumers of one set of facts that can silently
  disagree about an arity today, which is a worse failure than the magic was, because a wrong
  arity corrupts a stack and crashes somewhere unrelated. The complication is that the two
  hold them to different standards - a probe leaves out anything uncertain on principle,
  while an emulator wants the uncertain ones recorded - so a move has to carry the provenance
  level with the signature, not just the signature. See D003.

## Principles

### 1. Formats come from sources that can be named

No vendor headers, no SDK, no decrypted material, and **no format worked out by reading a
vendor binary**. Structure comes from published documentation and open-source implementations,
each cited by project and commit.

This is inherited from both sibling projects and it is the constraint that decides whether
this work can ever be shared. Reimplementation-from-a-binary converges on the original, and
that convergence is visible to anyone who looks.

### 2. Real files are an oracle, never a source

A package or an executable in hand is used to **confirm or refute** a structure taken from
cited sources. It is never used to derive one. The order is the whole distinction: derive from
something citable, then check against reality, and record which rows reality settled.

The material itself is never committed. What enters this repository is text with a header
naming what it came from, so somebody holding the same inputs can re-run the derivation and
somebody holding none of it can still see where every field was established.

### 3. The generation is in the type system

Two hardware generations share one container and differ in four bytes. A builder that does not
say which one it is targeting must not compile - not because it is tidier, but because the
one time this was a runtime parameter with a default, the default was wrong and the file was
rejected by the machine it was built for.

### 4. Read and write are one crate, and disagreeing is a bug

A format library that can only parse is half a library, and the half that writes is where the
errors are. Where both exist for one structure, a round trip is a test: parse what was
written, write what was parsed, and fail if they differ.

### 5. Nothing is invented

Where a field's meaning is unknown, it is named `unknown` and left alone. A plausible guess in
a container produces a file that is accepted and then read from the wrong place, which costs
far more than the gap it filled. An absent row in a table is visible; an invented one is not.

### 6. Fake, and it says so in a field

**The public fake-package keysets only.** Nothing here works on retail material and nothing
here should be made to.

Containers declare themselves fake in the field the format provides for it, and their signature
areas are zero. That has not changed: no vendor signature is forged, and none could be.

One thing *is* signed, and the distinction matters enough to write down. A package's licence
carries an RSA-2048 signature under the **debug RIF keyset** - a published keypair whose entire
purpose is the fake licences a non-retail package holds. Signing with it asserts "this is a
debug licence", which is true, and the licence says so in its own type field. The hardware accepts
it only where it accepts fake packages at all.

So the line is not "never compute a signature". It is: **never claim to be the vendor.** A
debug key saying *debug* is the opposite of a forgery, and refusing to use one would not have
made anything safer - it would only have meant a package this repository builds cannot install.
(D047)

## Working sessions

- Every non-obvious choice gets a numbered entry in `docs/DECISIONS.md` **as it is made**,
  with the reasoning, because the reasoning is what stops it being re-litigated.
- Append to `docs/WORKLOG.md` at the end of a completed unit of work. Record surprises
  especially - a format library's surprises are its most valuable output.
- Anything consulted goes in `ACKNOWLEDGEMENTS.md` in the same change.

## Where things live

- `data/` - the format tables. One row per field, with a provenance header naming every
  source by project and commit, and marking which rows real files settled. **These are the
  source of truth**; code reads them rather than carrying its own copy.
- `crates/` - a dependency spine, in build order: `abi` (the generation split, depends on
  nothing), `nid`, `elf`, `container`, `title`, `pfs`, `pkg`. Each depends only on those
  before it. The split is not organisational - it is what keeps cryptography out of a loader.
  An emulator reading a bare executable takes `elf` and stops.

  `title` sits off to one side: it depends on nothing in this repository and holds what a
  title says about *itself* - `PARAM.SFO` and `param.json`. `pkg` uses it because a package
  carries a `PARAM.SFO`; nothing else does.

  **This list must stay complete.** It was missing `title` for one crate's lifetime, and in
  that window a session read this file, concluded `PARAM.SFO` had no home, and wrote a second
  implementation of it inside `pkg` - hardcoding offsets the real one reads from
  `data/sfo-format.tsv`, and knowing nothing of the measured correction in D019. A crate
  absent from this paragraph is a crate the next session will write again. (D062)

  `nid` sits before `elf` rather than after it because a vendor module's undefined symbols
  are *named* by the hash - reading the symbol table and decoding the hash are one operation,
  not two. See D015.
- `docs/DECISIONS.md`, `docs/WORKLOG.md` - durable memory.

## The consumers, and what they are not promised

orbistoun, obSCEne and prosperous will depend on this. None of them gets an API shaped around
its own convenience: a format library that grows a method because one caller found it handy is
one that ends up encoding that caller's assumptions. If a consumer needs something shaped
differently, it wraps.

**obSCEne has migrated.** It deleted 2,801 lines - `nid.rs`, `dynlib.rs`, `module.rs`,
`mkself.rs` - and three of its `data/*.tsv` snapshots and its copy of `link/module.ld`, and
now takes these crates as path dependencies. Its own tag-derivation checker validates output
built here, and its published-pair self-test passes through the suffix that moved.

What it kept is the line this repository draws: the manifest saying which library resolves
which name, and the `$` sigil marking a symbol whose name *is* the identifier. Those are
conventions of a probe's symbol tables, not facts about a format.

**orbistoun has not**, and still holds the reading halves. Its `dynamic.rs`, `reloc.rs` and
`nid` are what `selfish-elf` and `selfish-nid` were built from.

### What the migration found, which is the argument for doing the next one

Four defects, and **two of them were here**:

- This crate's dynamic-table writer had silently dropped a module-version exception, so a
  previous-generation module would bind the wrong display library and draw a black window. Now
  `data/library-versions.tsv`, with a row and a test.
- `Tags::detect` decided the convention from the string table alone. `5` is also plain
  `DT_STRTAB`, so every ordinary shared object read as a current-convention vendor module.
  obSCEne's version was already right; this one now matches it.
- `Elf::vendor_segment` could not find the tables under the current convention at all. There
  is now `Elf::tables`, which handles both and rebases the offsets.
- obSCEne's `module.rs` carried a comment arguing for one `e_type` above code writing the
  other. It left with the file.

Two of four were this repository being wrong and a consumer being right. That is what a second
reader buys, and it is the same thing the two vendor tag ranges bought in the other direction.
