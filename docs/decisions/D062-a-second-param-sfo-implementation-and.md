# D062 - A second `PARAM.SFO` implementation, and why it happened


**A second `PARAM.SFO` implementation was written inside `selfish-pkg`, and the reason it
happened is more important than the duplication.**

`selfish-title` already read and wrote `PARAM.SFO`. It takes its offsets from
`data/sfo-format.tsv` - the stated source of truth - and carries a measured correction that
**overturned both of its cited sources** (D019: the key table pads to four, the file is not
padded at all, established against nine real files across three generations).

A session wrote a second one anyway: 582 lines, offsets hardcoded, knowing none of that. It is
the exact failure this repository exists to prevent, committed inside the repository built to
prevent it - and the argument in this project's own opening ("retyping a judgement into a new
language is how a transcription error gets into a census") applies to itself.

**Why it happened: `CLAUDE.md` did not list the crate.** The dependency spine named six of
seven. A session reading the onboarding document to find out what exists concluded, correctly
from what it was told, that `PARAM.SFO` had no home. The fix is therefore in two places, and
the second matters more:

- `selfish-pkg/src/sfo.rs` now holds only *which keys a package carries* - expressed through
  `selfish-title`'s API. That split is what `selfish-title`'s own header asks for: it declines
  to assert a required-key list because "a format crate that asserted one would report the
  first title that omits one as malformed", and names the consumer as the right place. A
  package is that consumer.
- `CLAUDE.md`'s spine is complete, and says out loud that a crate missing from it is a crate
  the next session will write again.

The duplicate was found by an audit, not by a test. Nothing failed: both implementations were
correct, and they happened to agree on the padding rule by luck rather than by derivation.

