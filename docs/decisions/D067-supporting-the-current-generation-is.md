# D067 - Supporting the current generation is three-quarters done and blocked on one oracle


The goal is for this crate to build for both console generations, with the current one added
alongside the previous rather than replacing it. An audit of where the `Generation` split
already reaches, prompted by getting an obSCEne package to install and launch on hardware,
found selfish is closer than it looks - and found exactly one thing it cannot do without a file
nobody here holds.

**Already generation-aware:**
- `selfish-abi::Generation` models both: container magic (`54 14 F5 EE` current, `4F 15 3D 1D`
  previous), abi version (2 vs 0), number (5 vs 4).
- `selfish-container` builds and parses either from the magic. obSCEne's `eboot.bin` already
  carries `54 14 F5 EE` - a **current-generation executable**, where the whole PS4-homebrew
  scene ships previous-generation ones. The hardest piece is the one already done.
- `selfish-title` reads and writes `param.json`, which the current generation uses in place of
  `PARAM.SFO`. A real one (`PPSA02664`) parses.

**The gap: the current-generation package format, magic `FIH` (`7F 46 49 48`).** selfish-pkg
recognises it only to refuse it (`UnsupportedFormat`, D021) and builds the previous-generation
`.CNT` format exclusively. Building `FIH` is what "current-generation package support" mostly
means, and it **cannot start yet**: principle 1 forbids deriving a format from a binary, and
principle 2 needs a real file as the oracle to confirm a structure taken from a citable source.
No `FIH` package is reachable - the console's `/data/pkg` and `/mnt/usb0` hold only `.CNT`
homebrew packages. A native title is installed, not distributed as a loose package, so its `FIH`
wrapper is not retained. **Getting a real `FIH` package is the prerequisite for this workstream.**

**What a native title actually is, measured from an installed one (`PPSA02664`, the oracle we
do have):** not a mounted package at all. The app directory holds a `mount.lnk` - literal text
`/mnt/usb0/PPSA02664-app0/$` - pointing at a decrypted app0 filesystem elsewhere, plus
`param.json` and a `sce_sys` richer than the previous generation's (`disc_info.dat`, `keystone`,
`nptitle.dat`, `pfs-version.dat`). So there may be a second path to a current-generation homebrew
title that skips `FIH` entirely: produce the installed form directly. That is groundable now
against this oracle, where `FIH` is not.

**Why the hybrid we have does not run, which is what motivated all this:** obSCEne is a
current-generation `eboot.bin` inside a previous-generation `.CNT` package with a previous-style
title id (`OBSC…`). That routes the launch through the PS4-compatibility path
(`scePs4AppCategoryGetForTitleId`), which refuses to exec a current-generation executable -
`preLaunchCheck 0x80a40086`, cleanly, after the pfs mount this session fixed succeeds. A coherent
title is both halves of one generation: previous+previous runs via compat with the previous
library surface, current+current would route to the native loader with the current surface.
selfish-pkg should carry the target generation and refuse a package whose executable disagrees
with it (principle 4) - the check that would have named this mismatch at build time. That, and
the installed-form path above, are the two additions that do not need the `FIH` oracle.

