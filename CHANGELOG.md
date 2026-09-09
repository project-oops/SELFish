# Changelog

SELFish ships as a **rolling build** plus tagged drafts - `main` refreshes one `latest-main`
prerelease, and a `v*` tag opens a draft versioned release. There is no semantic version yet,
so for anything off `main` the **short commit SHA is the version**.

Each entry is headed by the SHA (+ date) that shipped it, newest first. Within an entry,
changes are grouped **Added / Changed / Fixed**.

Nothing has shipped yet. This is the initial commit, so no entry below carries a SHA and the
CI that would produce one has never run.

## [unreleased] - as of 2026-09-09

### Added

- **Seven crates in a dependency spine**, in build order: `abi` (the generation split, and it
  depends on nothing), `nid`, `elf`, `container`, `title`, `pfs`, `pkg`. Each depends only on
  those before it, which is what keeps cryptography out of a loader: an emulator reading a bare
  executable takes `elf` and stops. `title` sits off to one side, holding what a title says
  about itself.
- **`selfish`, one command line** over all of it. `selfish <command> --help` is generated from
  the code, so it cannot drift from what the code does.
- **`data/`, the format tables**, one row per field with a provenance header naming every
  source by project and commit, and marking which rows real files settled. Code reads these
  rather than carrying its own copy.
- **The generation in the type system.** Two hardware generations share one container and
  differ in four bytes. A builder that does not name its target does not compile - because the
  one time this was a runtime parameter with a default, the default was wrong and the file was
  rejected by the machine it was built for.
- **The package and filesystem mount chain**, worked out far enough that a package this
  repository builds both installs and launches on real hardware.
- **Release workflow.** Windows, Linux and macOS archives, built through `./bin/selfish build`.
  No `clang` or `lld`: two integration tests need them and CI installs them for that, but a
  release only builds, and a toolchain a build does not use is a dependency that fails one day
  for no reason anybody can trace.

- **Reading an SFO parameter as bytes.** `Sfo::bytes(key)` and `Value::as_bytes` return a
  parameter as the file holds it, for every kind but a number. `ACCOUNT_ID` is eight bytes of
  user id in the unterminated format, and it is what a consumer retargeting a save needs -
  never endian-swapped, never rendered as a number, because which end of a user id is
  significant is the caller's question. New `Value::Binary` for a value in that format that is
  not text. Requested by Prosperous.
- **`selfish audit` says what kind of container it agreed with**, above the row count.
  `Audit::declared` reads `ex_info.ptype` and reports it; a container that declares itself fake
  agrees with this table because it was written from one, so a match there is a round trip and
  not evidence about vendor material. That reading was got wrong here first, against real
  hardware output, and cost a reversal. (D092, D093)
- **The audit checks the tail as well.** `Audit::tail` and `tail_differing()` cover the four
  `ex_info` rows the table pins, reported under their own heading rather than folded into the
  header count - a header row differing is a claim about the format, a tail row differing
  usually just means "not a fake container". (D094)
- **`name_nid`**, a probe that runs the import hash backwards: given an identifier, hash a
  vocabulary and find a name that produces it. The vocabulary is passed **by path** - a mined
  corpus belongs to the project that mined it - and every byte-order reading is printed with
  its encoded form, because this collection holds two conventions that disagree. (D089)
- **`sfo_params`**, printing every parameter in a `param.sfo` with its format, the variant it
  read as, and named keys as hex.
- **`depended_on`**, a fixture of identifiers a sibling project has copied into its own source,
  so a change to the hash fails here rather than in their build.

### Changed

- **obSCEne migrated onto these crates**, deleting 2,801 lines and three of its own format
  snapshots. The migration found four defects and **two were here**: a dropped module-version
  exception that would have bound the wrong display library, and a convention detector that
  read every ordinary shared object as a vendor module. A second reader is what bought those.

- **The builder refuses a symbol name it cannot read**, where it used to guess.
  `dynlib::string_at` returned `""` for an out-of-range offset, the rest of the table for an
  unterminated one, and a lossy `U+FFFD` for invalid UTF-8 - and its result is *written back
  into the module*. New `BuildError::SymbolName(u32)` carrying the offset, because the usual
  cause is the wrong string table rather than the wrong byte. Every existing test passed
  unchanged, so nothing real was relying on any of the three. (D091)
- **`data/self-format.tsv` records what hardware measured**, and what it does not settle. Nine
  containers were audited on a current-generation console; the tail layout `header_size - 0x70`
  held on seven this repository did not write, which is the first result the table has from
  such material. The `nothing here is confirmed for the current generation` limit **stands** -
  the container that appeared to lift it was fake.
### Fixed

- `D086` was written twice by two sessions. The on-console audit keeps the number, because
  three obSCEne source files and D085 cite it; the `libkernel_vaddrs` entry moved to D088 and
  says so in its own text, so a stale citation still lands somewhere.
- **A save carrying `ACCOUNT_ID` would have failed `Sfo::parse` outright.** Everything in the
  unterminated format went through `from_utf8`, so an id that did not decode returned
  `NotUtf8` for the whole file - not the key - and a caller got nothing else out of it either.
  Underneath it, trailing zeroes were being trimmed from a format that has no terminator and
  therefore an exact length, so an id ending `0x00` came back a byte short. (D090)
- **D092 is reversed.** It recorded the header confirmed at the current generation by a retail
  vendor game; the container's `ex_info` is this repository's own defaults, `ptype` `0x1`, the
  fake marker. A file written from this table agreeing with this table is a round trip. The
  entry keeps its text - the `vendor` that misled it is a hardcoded label in a probe's
  candidate list, and the row above it in the same array had just been checked for exactly
  that and written up.
