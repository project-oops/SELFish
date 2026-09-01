# D021 - There is a second package magic, and it is named and refused rather than guessed at


`\x7FFIH` alongside `\x7FCNT`. The console's installer accepts both; `selfish-pkg` reads only
the second, and now says so with its own error.

Two failure modes were available and both are worse than an error:

- **Reporting it as not-a-package.** False about a file that works, and the sort of thing that
  sends somebody looking for a fault in their package rather than in the tool.
- **Assuming the same layout.** The header is big-endian, so reading a count and a table
  offset out of the wrong header still yields a count and a table offset. Entries would be
  produced. None of it would be right and none of it would be detectably wrong - which is
  precisely the shape principle 5 exists to prevent.

The magic is in `data/pkg-format.tsv` marked *layout NOT established*. That is the whole
entry: knowing a format exists is worth recording separately from knowing how to read it.

