# D059 - A `param.sfo` is a format and belongs here. An icon is a picture and does not


Both were being shipped as the ASCII word `PLACEHOLDER`, and both had been filed under "the
title's content, which a format library must not invent". Only one of them was.

`PSF` is a format - a magic, an index, a key table and a data table, with ordering and alignment
rules that are wrong in exactly the way a wrong format is wrong. A package carrying twenty-one
bytes where the magic goes fails on the first four. `selfish_pkg::sfo` writes a real one, and
reading a real one back is what proves it: `examples/param.rs` parses the `param.sfo` out of
three real packages - recovering their actual titles - and confirms **every field this crate
writes has the same type and the same field width as the real files. 3 of 3.**

What the values *are* stays the caller's. `Params` has no default for the title or its id.

An icon has no encoding this repository owns and no derivation from anything. `PNG` is a format
but it is not one of *the console's* formats, so by the admission test it fails on the first
question. It stays an input.

**But an installer still needs a file it can decode**, so the *tool* - not the library -
generates one when none is given, and says so on stdout. That placement is the whole
distinction: `selfish-cli` is a convenience, `selfish-pkg` is knowledge.

