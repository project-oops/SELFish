# D057 - The flat path table is real now, and it was built precisely because nothing here reads it


It was a placeholder: a list of paths under the right name. Every test passed, because a reader
walks directory entries and never consults the table - so a wrong one fails only on the machine
it was built for, which is the worst possible place to find out.

The format is a hash of each path mapped to its inode, sorted, with a bit marking directories.
The hash is the familiar `31 * h + c` over an **upper-cased** path, which is why two files
differing only in case collide. Collisions need a second file this does not write, so
`has_collision` answers the question before building and `build` refuses rather than emitting a
table with an entry silently missing.

