# D054 - The key blobs are computed from the public keys

**Status:** decided
**Date:** 2026-09-26

Package entries `0x10` and `0x20` are computed from the content id and passcode, wrapped under
the public halves of the package keys in `data/pkg-keys.toml`, whose moduli are extracted from
`LibOrbisPkg/Util/Keys.cs` by a parser rather than by hand.

**Why:** the blobs carry the filesystem key the hardware unwraps, so a package without correct
ones cannot be opened. Wrapping needs only public keys, so producing them grants no ability to
read anything. The padding is deterministic, so real packages check the result byte for byte.

**Rejected:**
- Requiring the caller to supply them: a caller with nothing to hand supplies zeros.
- Transcribing key material by hand: transcription of keys has failed repeatedly.
