# D060 - The default icon is selfish's own mark, reversing D059


**The default icon is selfish's own mark, not a blank tile. That reverses the call in D059 and
the reversal was right.**

D059 emitted a single blank pixel, reasoning that a branded default puts this project's
identity into other people's packages and that branding belongs to the consumer.

That was the wrong trade, and the argument against it is short: **a blank tile on a home screen
tells you nothing.** A recognisable one tells you two things at once - that selfish built this
package, and that nobody supplied an icon. Both are true, both are worth knowing, and the place
you most want to know them is a console you cannot attach a debugger to. The purity argument
was protecting nothing; the diagnostic is real.

`selfish-cli/src/icon.rs` draws `SELFISH` in a frame: 512×512, indexed, one bit deep, about
33 KB. **Drawn rather than committed** - a glyph table and a few loops, so it reads and reviews
in a diff like everything else here, and a real logo can replace it wholesale later without
anything else moving. It is still one flag to override.

Two things the tests could not have told us, found by decoding the result and looking at it:
the word overflowed the frame at the first scale tried, and the encoding had to be indexed
rather than truecolour to stay small. A CRC check passes happily on a picture that says the
wrong thing.

The same reasoning makes the playgo manifest a generated default: it is a constant string,
identical in every package examined, so there is nothing to invent.

**What is still handed over, deliberately:** the entry name table and `playgo-chunk.dat` are
passed empty. If a console wants either, that shows up as a specific rejection rather than as a
guess that installed.

