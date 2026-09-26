# D073 - The CLI owns artwork

**Status:** decided
**Date:** 2026-09-26

Artwork is handled by `selfish-cli`, not the libraries. A supplied icon is composited over
black, flattened to 512x512 RGB, and refused if it is another size; the background and logo are
normalised the same way at their own sizes. With nothing supplied, the tool uses selfish's own
mark from `assets/logo.svg` (rasterised to `assets/logo.png`), and says so.

**Why:** PNG is not one of the hardware's formats, so the libraries do not own it, but four
projects build packages and each would convert differently. An icon with alpha is accepted and
then composited wrongly rather than refused. Scaling is a judgement about the artwork. A
recognisable default says both that selfish built the package and that no icon was supplied.

**Rejected:**
- Per-project full-bleed variants: four copies of one requirement.
- Resizing: which filter to use is the artist's decision.
- A blank default tile: carries no information.
