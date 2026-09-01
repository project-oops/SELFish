# D070 - The cache size is a ceiling measured from the image, not a constant


> **Superseded by D071.** The rule below is wrong: the declared cache size does not decide it, and
> the clamp this entry introduced has been removed. What is real is that the *inner filesystem*
> must exceed a fixed size. Kept because the measurements are sound and only the conclusion drawn
> from them was not.

`CACHE_SIZE` at `0x43C` was written as `0xD0000` unconditionally, because that is what all three
real packages carry. All three are also tens of megabytes, and the constant turned out to encode
a constraint nobody had stated.

A minimal package - built to shrink a reproducer - has an inner filesystem of `0xB0000`, which is
**smaller than the cache its own header declares**. A console mounts the outer image, opens
`pfs_image.dat`, and then refuses:

```text
sceFsMountGamePkg(1452) ***ERR*** Failed to enable GDDR5 cache.
sceFsMountGamePkg() ret = 80020016            EINVAL
```

Padding the inner image to `0x1B0000` cleared the error outright. That is what makes this a
measured rule rather than a reading of the name: **the declared cache may not exceed the inner
filesystem.**

`Builder::cache_size` now overrides the default, and `selfish pack` computes the inner size and
clamps when it is smaller, saying so on its output. The comparison has to be against the *inner*
filesystem: the minimal package's outer image was 1,179,648 bytes, comfortably above `0xD0000`,
so a check against the image this crate is handed would have passed and changed nothing.

**Clamped, not refused.** A caller with a small title has done nothing wrong, and a builder that
refuses to produce one would be enforcing a limit that only exists because of a constant this
crate chose. What the field means beyond the ceiling is still unknown and nothing here invents a
formula for it - `DEFAULT_CACHE_SIZE` remains the measured value for everything large enough.

**What this is really an entry about.** Three packages agreeing on a value says what a big title
writes, not what a field means, and the agreement is most convincing exactly where the sample is
least varied. Every constant in `header_value` came from the same three large packages, so each
is a candidate for the same mistake; the ones that scale with the image are the ones to suspect
first. Reaching for the sample size as evidence - *3/3* - was the specific error, because three
identical inputs are one measurement repeated.

The trip that found this also cleared a suspect: rebuilding with `pfs_image.dat`'s compressed bit
off and `size_compressed` equal to `size` produced the identical error at the identical line, so
that field causes none of it. Recorded because it was the obvious candidate and would otherwise
be re-suspected.

