# D051 - The two ways of writing a `1` near the end of the superblock are two fields, not one


An image *with* a seed writes an index at `0x36C` and the seed after it. An image *without* one
writes `1` at `0x368` and stops. Both had been measured as "a 1 near the end of the header",
and reading could never tell them apart because nothing reads either.

The writer is what separated them, and it is the second time in this area that writing has
found something reading could not (D042 was the first). Both offsets are now named. The inner
filesystem is unseeded and uses `NO_SEED_INDEX`; the outer one is seeded and uses
`UNKNOWN_INDEX`.

The same pass corrected `N_BLOCK`. It is not the block count, despite the name: the source
writes a literal `1` and remarks that it always is. Writing the real count there would have
been more plausible and would have made every image this builds differ from every image
examined, in a field nothing checks.

