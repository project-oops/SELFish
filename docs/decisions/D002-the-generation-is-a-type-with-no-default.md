# D002 - The generation is a type with no `Default`


Two generations share one container and differ in four bytes. Everything else at this layer
is identical, which is what makes it dangerous: the wrong one is structurally perfect and
refused on the first four bytes, reported by a loader as "not a container".

It was a runtime parameter with a default once. The default was the previous generation,
because that is what every published source describes, and the current generation's magic had
been observed and written down somewhere else entirely.

So `Generation` has no `Default` and the magic is returned as `[u8; 4]` rather than a `u32`.
The second half matters as much as the first: held as an integer the constant looks right and
serialises backwards, which also happened, and was caught only because a test asserted on the
written bytes rather than on the constant.

Status: **decided**.

