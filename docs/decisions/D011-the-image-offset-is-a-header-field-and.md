# D011 - The image offset is a header field, and the evidence that it was not could not have failed


`IMAGE_OFFSET` was recorded as a fixed `0x700000` - a value taken from a previous-generation
extractor, "confirmed" by finding high-entropy data at that offset in all three samples.

That confirmed nothing. In an encrypted package *almost every offset* holds high-entropy data,
so the test could not have come out any other way. It was a proposition dressed as a
measurement.

The real values are `0x80000`, `0x580000` and `0x80000`. Not fixed, not `0x700000`, and named
in the header all along - a big-endian `u64` at `0x410`, with 32-bit mirrors at `0x7C` and
`0x414` in every sample. Three samples cannot rank the three, so the widest is read and the
others are recorded.

### The lesson, which is about method rather than about packages

The original search asked *"is `0x700000` mentioned in the header?"*, found nothing, and
stopped. The question that settles it is *"where does the image actually start, and is **that**
in the header?"* - and it was, immediately.

Searching for a value you already believe, and treating its absence as support for believing
it, is not a measurement. The corrected constant carries that story in its doc comment so the
next person meets the reasoning and not just the number.

Status: **decided**.

