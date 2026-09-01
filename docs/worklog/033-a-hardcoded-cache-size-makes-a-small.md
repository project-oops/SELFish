# A hardcoded cache size makes a small package unmountable


`CACHE_SIZE` at `0x43C` is written as a constant, `0xD0000`, because that is what all three real
packages carry. All three are also tens of megabytes. A minimal package - built to isolate a
console panic by shrinking the image - has an inner filesystem of `0xB0000`, which is **smaller
than the cache its own header declares**, and a console refuses it:

```text
[PFS] mount finished 1 1 1 1 0 1 0 0          the outer image mounts
sceFsMountGamePkg(1452) ***ERR*** Failed to enable GDDR5 cache.
sceFsMountGamePkg() ret = 80020016            EINVAL
```

The outer image mounts and `pfs_image.dat` opens - the failure is 27 lines further into
`sceFsMountGamePkg` than the `ENOENT` that the inode-flag fixes cleared. Then it asks for a cache
larger than the thing being cached, and stops.

Two things worth keeping from this:

- **A constant measured only from large samples is a constraint nobody stated.** Three packages
  agreeing on `0xD0000` says what a big title writes, not what the field means. Anything derived
  from a sample that varies in one direction only should be suspected the first time it is used
  outside that range.
- **It is not the compressed bit.** Rebuilding with `pfs_image.dat`'s compressed flag off and
  `size_compressed` equal to `size` produced the *identical* error at the identical line, which
  clears that field of causing this one. A clean negative, and worth the trip: the flag was the
  obvious suspect and it is innocent.

The consequence for the investigation is larger than the bug. A minimal package was built to test
whether a console panic during the app0 mount came from the filesystem writer or from the payload
inside it - and it turns out a small package fails *earlier*, for a reason of its own, so it never
reaches the code that panics. **It is not a reproducer.** A useful one has to stay above the
declared cache size, which is the opposite of minimal.

