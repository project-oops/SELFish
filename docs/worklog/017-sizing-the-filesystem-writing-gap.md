# Sizing the filesystem-writing gap instead of starting it


`writing the filesystem` was next on the backlog, and it turned out to be the wrong thing to
start. `examples/superblock.rs` asks how much of a superblock a writer would have to produce
that nothing here can name.

Three real packages, identical answers: **38 of 1024 bytes carry a cited field; 95 further
bytes are non-zero and unaccounted for**, in sixteen runs. Two of the runs are thirty-two bytes
wide and sit where a digest would - a shape, not a meaning.

So this is the reader-only-source problem, not effort remaining, and the backlog now says so.
The container spent months in this exact position and was unblocked in an afternoon when an
open-source *writer* turned up; a reader walks a filesystem by following four numbers and never
looks at the other 95 bytes, so no number of readers will supply them.

The example stays because the question recurs. It answers one, it does not derive a format:
nothing from it went into `data/`.

