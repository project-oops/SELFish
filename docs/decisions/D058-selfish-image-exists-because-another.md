# D058 - `selfish image` exists because another session asked for it in a script comment


obSCEne's `scripts/build-pkg.sh` was written by a concurrent session against a selfish that
could not yet build an image, and it said exactly what it needed: *"Add e.g. `selfish image
--root DIR --out IMG` (or a directory mode on `pack`)"*. Both now exist.

Its call needed one correction that is worth recording, because it is the kind of mistake that
produces two files which each look fine: `image` takes `--content-id`, and it has to be the
same string the package declares. The image is encrypted under a key derived from it, so a
mismatch yields a package whose filesystem cannot be opened by anything, with nothing visibly
wrong in either half.

