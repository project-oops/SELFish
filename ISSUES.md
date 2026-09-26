# Issues

Open defects, gaps and unmeasured facts, one line each. Delete a line when it is fixed.

- The current-generation package format is not written.
- A package's generation is not checked against its executable.
- Doubly-indirect PFS signature blocks are unsupported.
- Flat-path-table hash collisions are not resolved.
- The digest slot at `0x80`/`0x60` is filled after the header is written, so the builder lists it as a gap.
- A `.prx` cannot be wrapped in a container.
- ABI declarations are not shared under D003's provenance condition.
- `vendor::EXPORT_LIB_PROSPERO` (`0x6100_004D`) is unconfirmed.
- `data/self-format.tsv` rows cite sweep IDs in their note column; `data/agc-shader-format.tsv` rows cite `(D002)` and "the draft".
- `data/sdk-versions.toml` carries `description = "Zero SDK for D244 census probing"`, which the CLI prints.
- `ci.yml` has no bootstrap step (`tools/check-workflows.sh`).
