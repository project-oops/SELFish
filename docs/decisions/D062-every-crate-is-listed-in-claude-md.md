# D062 - Every crate is listed in CLAUDE.md

**Status:** decided
**Date:** 2026-09-26

`PARAM.SFO` has one implementation, in `selfish-title`, read from `data/sfo-format.tsv`.
`selfish-pkg` decides only which keys a package's `param.sfo` carries, through that crate's
API. `CLAUDE.md` lists every crate on the spine, and a new crate is added to it in the same
change.

**Why:** a session that cannot see a crate from the onboarding document writes it again, with
hardcoded offsets and without the corrections the first one carries.

**Rejected:**
- A package-local `PARAM.SFO` writer: a second copy of a format this repository owns.
- Relying on search to find crates: a reader starts from `CLAUDE.md`.
