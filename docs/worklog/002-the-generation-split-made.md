# The generation split, made unrepresentable rather than tested for


`Generation` has no `Default`, and `container_magic()` returns `[u8; 4]` rather than a `u32`.

Both are shaped by the bug that caused this repository. A builder emitted the previous
generation's magic because that is what every published source describes, while the current
one had been observed and recorded in a sibling project that nothing connected it to. Then,
fixing it, the constant went in as `0x5414F5EE` - which looks correct and serialises
backwards. Caught only because a test asserted on written bytes rather than on the constant.

So: a caller cannot omit the generation, and there is no integer to get the wrong way round.

