# D014 - Reassembly is the inverse of building, and both live together


`Container::to_elf` puts a scattered executable back: a container stores its inner headers
after the entry table and each segment wherever an entry says, so recovering it means writing
every segment to the offset its *program header* names.

Sized by the furthest any program header reaches rather than by the container's length. The
container is larger, and the difference is metadata that has no business in a reassembled
executable.

An entry whose segment index is past the program header table is **skipped rather than
fatal**. A partial executable beats none, and a container describing a segment the executable
does not have is a finding worth surviving to report.

It closes the loop that mattered: a real vendor library, extracted from a real package,
unwrapped and read - 2,676 symbols and four named import libraries. Until this existed, every
container the reader had seen was one its own writer produced.

Status: **decided**.

