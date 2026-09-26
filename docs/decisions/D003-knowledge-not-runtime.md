# D003 - Knowledge, not runtime

**Status:** assumed
**Date:** 2026-09-26

This repository holds what a consumer needs to know about a format - layouts, the import hash,
segment layout rules, linker scripts - and nothing that runs on the hardware: no `crt`, no
allocator, no C library. Platform ABI declarations may move here only with each signature's
provenance level attached.

**Why:** the risk is a repository that accumulates a runtime and becomes where everything
lands. The line sits at knowledge against runtime. ABI declarations are held to different
standards by a probe (omit anything uncertain) and an emulator (record it anyway), so a shared
set must carry the level or it imposes one project's standard on the other.

**Rejected:**
- "Not an SDK": excludes linker scripts, which are format knowledge.
- Moving ABI declarations as a plain list: drops the provenance level each consumer relies on.
