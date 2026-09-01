# D001 - A fourth repository, holding formats and nothing that knows what a consumer is for


Three projects need these formats and each had grown its own copy. The cost was not
hypothetical: a container builder shipped emitting the previous generation's magic, because
the current one was recorded in a sibling project's decision log and nothing connected the
two. The file would have been rejected by the only machine it was built for, on its first
four bytes.

The duplication argument alone would have been arguable - a reader and a writer are genuinely
different code, and merging them buys a coordination tax. What settles it is that the
duplicated part is the **facts**, not the code: the header layout, the field offsets, the flag
bits and the magic values each existed more than once, and one copy was wrong.

**The second reason is contention.** A repository is a unit of concurrent work. While these
formats lived inside a probe's tooling directory, work on a package writer and work on the
probe were the same working copy, so they serialised - one session's refactor blocked the
other's feature for an afternoon. Splitting removes a duplication *and* a queue.

Status: **decided**.

