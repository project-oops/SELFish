# D073 - The tool converts a supplied icon, rather than asking four projects to export one


A package's `icon0.png` has to be 512x512 with **no alpha channel**. All three real packages
examined are colour type 2 with their artwork running edge to edge, and the rounded corners on a
home screen are the console's own mask laid over the top.

An icon that does not match is **not rejected**. It is accepted, installed, and then composited
differently: artwork exported with a transparent margin sits inset inside it, and the tile reads
square beside every other icon on the screen. That is a format requirement you discover by looking
at a television, which is the worst place to keep one.

The first fix was a per-project full-bleed variant, generated beside each logo. That was the wrong
place and it did not survive being questioned: four projects here build packages, so a variant per
project is four chances to get one requirement wrong, four files to regenerate when a logo
changes, and no single place to correct it. The requirement is written down here; the conversion
belongs here too.

So `selfish pack` normalises whatever `--entry 0x1200=FILE` hands it - composites transparency
over black, flattens to RGB, and says on its output that it did. A caller supplies the picture
they have and gets the picture a console wants.

**It does not resize.** A wrong size is refused, with the actual size in the message. Scaling is a
judgement about somebody else's artwork - which filter, whether to letterbox a non-square image,
whether to sharpen pixel art that nearest-neighbour keeps crisp and bilinear turns to mush - and
guessing silently is how a logo ends up blurred with nobody able to say which step did it.
Refusing names the problem where the artwork is, which is the only place it can be fixed properly.

**Transparency is composited, not dropped.** Dropping an alpha channel leaves whatever colour
happened to sit under a transparent pixel, which on antialiased artwork is white fringing.

This adds `png` - decode, composite, encode, and nothing else. A full imaging crate would bring
filters, formats and colour management to do one conversion, and the trade is the same one that
put `sha1` and `aes` in this workspace rather than hand-rolling them.

