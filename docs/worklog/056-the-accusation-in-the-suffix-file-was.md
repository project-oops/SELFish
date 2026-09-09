# 2026-09-09 - The accusation in the suffix file was made from a label


`data/hash-suffix.toml` has said, since it was written:

> **Little-endian.** The emulator side of this project documents it as big-endian; that is
> incorrect, and the test vector above is the evidence.

Left alone earlier today as "stale, and I would rather fix it against their actual text than
from an inference about it". Reading their actual text was the right instinct and it found
something better than staleness: **the sentence was wrong when it was written.**

## What their source says

`orbistoun-nid`'s `hash` and `hash_bytes` sit one above the other. `hash` delegates straight to
`hash_bytes`. Their docs disagree:

```
hash:       "The first eight bytes of the SHA-1 digest, read little-endian"
hash_bytes: "**Big-endian**, and this was wrong for a long time. Reading the digest
             little-endian produces a perfectly plausible hash that agrees with nothing"
```

and the code is `u64::from_be_bytes`. So the contradiction orbistoun asked about in
REQ-20260909T1910Z-3d05 - *"whichever of the two docs is describing the other convention is worth
correcting"* - is inside their own file, between two adjacent functions, and the stale one is
`hash`. That answer was not available when their request was resolved, because it was answered
without reading their source.

## And the mirror

Their `hash_bytes` note records exactly the discovery this file records, reversed: they found
little-endian "agrees with nothing", this file found big-endian does. **Both are true locally
and both were written as though universal.**

Neither is a bug. Orbistoun reads the digest big-endian and hashes big-endian to match, so its
`u64` is the byte-reverse of this one and every operation on it is self-consistent - which is
why their name search resolves, and why the differential found all 29 modules matching once the
reversal was accounted for. The `u64` is an internal representation. Two projects unpack the same
eight bytes into it differently and both arrive at the same eleven characters.

`sceKernelLoadStartModule -> wzvqT4UqKX8` is a published pair and **both** produce that string.
That is the shared truth, and the file now says to exchange it rather than the number.

## The pattern, for the last time today

This is the fourth: a convenient attribution taken on trust (D092), a pessimistic reading taken
on instinct (worklog 051), somebody else's self-doubt endorsed for free (D095), and now an
accusation carried in a data file for however long, about a project whose source is on the same
disk.

Every one was a judgement about what something *meant* made from what it was *called* - a probe's
candidate label, a verdict's headline, a field name, a doc comment. The check that resolves all
four is the same and it is cheap: **go and read the thing.** Twice today it took under a minute.
