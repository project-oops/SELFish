# D068 - The logo is one file, committed, and the drawn mark is gone


D060 put selfish's own mark on every package that arrives without an icon, and drew it here
rather than committing one: a glyph table and a few loops, so it reads in a diff like everything
else. It also said what should happen next, in as many words - *when there is a real logo it can
replace `default_icon` wholesale; nothing else has to move*.

There is now a real logo, so that is what happened. `assets/logo.svg` is the source of truth and
`assets/logo.png` is a 512×512 raster of it, embedded with `include_bytes!`. The glyph table, the
border, the word and the hand-rolled indexed-PNG encoder are deleted.

**The reason to do it wholesale rather than keep both is that two logos diverge.** A drawn mark
in the tool and a real one in the readme stay identical exactly until either changes, and the one
that ends up on a console is the one nobody is looking at. One asset, one place.

What is lost is real and worth naming: a committed binary cannot be reviewed in a diff, which was
D060's argument for drawing it. Two things make that an acceptable trade rather than a quiet
regression. The SVG *is* reviewable - it is text, and it is the source of truth, so a change to
the mark shows up as a readable diff. And the PNG is regenerated from it by a recipe written down
out of band with any SVG rasteriser and committed. Its `viewBox` crops to the artwork and its
corner radius matches the mask a console lays over every home-screen icon, so both live in the SVG
rather than in a build step.

Two tests stand in for the review the diff no longer gives: the embedded bytes start with the PNG
signature, and `IHDR` still says 512×512. Both exist because the raster is produced by a step
outside `cargo`, and a truncated or wrongly-sized asset would otherwise reach a package and
surface as a tile that will not draw.

**A rasteriser is not being added to this repository.** Nothing here rasterises SVG, a formats
library is the wrong home for one, and the cost of the alternative is one 3 KB file and a
documented command. The recipe records the trap that cost the first attempt: a `file://` `fetch`
of an SVG is blocked by CORS, and the rasteriser writes a well-formed *blank* PNG that is about
the same size as the real one, so "it produced a file" is not evidence it worked.

