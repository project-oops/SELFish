# D066 - Documentation drifts in the direction of claiming less than the code does


The audit turned up four stale claims, and every one understated what was true:

- `README.md`: *"Signing is not attempted and could not be."* `CLAUDE.md` principle 6 had been
  updated for the debug RIF licence signature (D047) and the README had not, so the two
  documents **contradicted each other on a safety-relevant claim**. The README also documented
  `pack` without `--dir`, `--passcode` or `--title-id`, and omitted `selfish image` entirely.
- `data/pkg-keys.toml`: opened with *"Two RSA-2048 keypairs"* while holding three keypairs, an
  AES key and seven public moduli. A provenance header that undercounts its own file is worse
  than none, because it is read as authoritative.
- `docs/BACKLOG.md`: items 4 and 5 described the filesystem as unwritten and the package writer
  as lacking an image, both long since done - and listed the 32 bytes at `0x380` as unexplained
  when D052 had identified them as the header's own block signature.
- `keys.rs`: the `DK3_RANGE` doc said the key index *"does not agree"* with the offset and that
  *"nothing here establishes which way the index maps onto the blob"*. D054 established exactly
  that, several hundred lines lower in the same file. The range is now **derived** from
  `dk3_block_at()` so the reader and the writer cannot drift apart.

The pattern is worth naming. Code that gains an ability rarely revisits the paragraph that said
it lacked one, so documentation decays toward pessimism - and a reader who trusts it either
rebuilds something that exists, or reasons from a limit that has been lifted. That is not
hypothetical here: it is what produced D062.

