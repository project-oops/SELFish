# Changelog

SELFish ships as a **rolling build** plus tagged drafts - `main` refreshes one `latest-main`
prerelease, and a `v*` tag opens a draft versioned release. There is no semantic version yet,
so for anything off `main` the **short commit SHA is the version**.

Each entry is headed by the SHA (+ date) that shipped it, newest first. Within an entry,
changes are grouped **Added / Changed / Fixed**.

Nothing has shipped yet. This is the initial commit, so no entry below carries a SHA and the
CI that would produce one has never run.

## [unreleased] - as of 2026-09-01

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

### Changed

- **obSCEne migrated onto these crates**, deleting 2,801 lines and three of its own format
  snapshots. The migration found four defects and **two were here**: a dropped module-version
  exception that would have bound the wrong display library, and a convention detector that
  read every ordinary shared object as a vendor module. A second reader is what bought those.

### Fixed

- `D086` was written twice by two sessions. The on-console audit keeps the number, because
  three obSCEne source files and D085 cite it; the `libkernel_vaddrs` entry moved to D088 and
  says so in its own text, so a stale citation still lands somewhere.
