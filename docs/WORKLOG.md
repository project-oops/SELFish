# Worklog

One entry per milestone. Commit messages hold the rest.

## 2026-08-29 - Every format read end to end

- The container, ELF with both vendor tag conventions, symbols, imports resolved to library and
  module, relocations and section headers read from real material.
- A real package opens through the whole chain: outer container, key derivation, encrypted
  filesystem, `PFSC`, and the inner filesystem's files.
- `PARAM.SFO` and `param.json` read and written; eleven real files round-trip byte for byte.
- The import hash reproduces all 389 externally produced pairs in `known-pairs.txt`.
- Every current-generation package sampled carries previous-generation containers, and the
  previous-generation key chain opens all of them.

## 2026-08-29 - Modules and packages written

- `dynlib` writes the vendor dynamic tables into a module linked with `link/module.ld`; a test
  links a real object and reads it back.
- The whole filesystem nest is written: the plain inner filesystem, `PFSC`, and the signed and
  encrypted outer filesystem, checked against the reader.
- The package builder computes both digest tables, the block digests, the manifest, the header,
  the licences and the key blobs; licences and key blobs reproduce real packages byte for byte.
- obSCEne took these crates as dependencies in place of 2,801 lines of its own.

## 2026-08-29 - A package built here runs on hardware

- A package built entirely by this toolchain installs, mounts, loads and executes.
- A container entry's size is the data's size, not `p_memsz`, which is zero for the unmapped
  vendor segment.
- The dynamic table sits at the tail of the vendor segment with no address.
- The vendor segment starts with a `0x18`-byte fingerprint region, written as zeros.
- An executable declares no export library, so its first import library is id zero.
- The inner filesystem's root is its own parent, and its inodes carry the flags the mount reads.

## 2026-09-09 - Real containers audited, and a second reader agrees

- `selfish audit` checks a real container's header and tail rows against the table, printing
  the container's kind first.
- 23 genuine containers confirm four header rows the table held as hypotheses; the values that
  differ are recorded as notes on their rows.
- orbistoun's reader and `selfish-elf` agree on every field of 29 real modules.
- `Sfo::bytes` returns non-text values such as `ACCOUNT_ID` exactly, and the module writer
  refuses names it cannot represent.
- `examples/name_nid` searches a caller's vocabulary for a hash in both byte orders.

## 2026-09-10 - One build pipeline

- `--input`, `--target`, `--format` and `--output` build every artefact: `elf`, `prx`,
  `eboot`, `title` and `pkg`. No stamping or wrapping subcommands remain.
- `--privilege`, `--sdk` and the title metadata options reach the formats that use them and are
  refused elsewhere.
- Entries `0x200` and `0x1001` are computed, so a package needs no `--entry`; the playgo chunk
  table was confirmed on hardware.

## 2026-09-11 - Shader containers

- `selfish-shader` builds the AGC shader container from five agreeing open-source sources, and
  `selfish shader` exposes it to C consumers.
- Compute, pixel and vertex stages have citable type values; a hardware sweep perturbing one
  field at a time confirms the header.

## 2026-09-23 - C++ titles catch exceptions

- `link/native_eboot.ld` keeps `.eh_frame`, so a title linking libunwind has a frame table and
  its `catch` blocks run on hardware.
- The check that means something is a non-empty `__eh_frame_start`..`__eh_frame_end` range.
