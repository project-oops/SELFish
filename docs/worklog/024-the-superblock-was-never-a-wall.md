# The superblock was never a wall


Fetched `LibOrbisPkg/PFS/PfsStructs.cs`. It names the whole header, and every field checked out
against three real images on the first try.

D027 had measured the gap correctly - 38 of 1024 bytes cited, 95 non-zero and unaccounted for -
and then drawn the wrong conclusion: blocked on a source. The bytes were never unknowable, only
unnamed. That is the second time the same mistake has been made and corrected this session, and
the tell both times was the same: a *reader* only follows the handful of numbers it needs, so
everything else stays anonymous and starts to look like a wall.

The `0xb8` field measured as "33 non-zero bytes of something" is an inode structure describing
the inode block. The seed is **zero in all three images**, which is worth knowing before
relying on the derivation that hashes it.

Two invariants came out of it, and they check each other: `data_blocks * block_size` is the
image length exactly - 655, 951 and 1152 blocks - and that same count is where a package's
`PLAYGO_CHUNK_SHA` table starts describing the image, derived days apart from a different
structure.

### The gate was hiding a failure

`cargo test --workspace | grep -oE '[0-9]+ passed' | awk sum` throws the exit code away. A run
that aborted on a failing test just came back with a smaller number, and 162 quietly became 116
without anything saying so.

That is precisely the defect obSCEne's `verify.sh` was rewritten to remove - *"output filtering
never sits between a command and its exit code"* - reintroduced here by hand. The gate now runs
under `set -e` with the filtering applied to a file afterwards.

The failure it was hiding was a good one: adding a magic check meant an all-zero superblock now
fails on the magic rather than on a zero block size, so the test asserting the latter had
stopped testing it. Both are covered now.

