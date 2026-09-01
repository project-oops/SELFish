# Crunching the unknown package entries


Pushed on "can't we just guess", and the push was right. The distinction that matters is
whether a guess can be **falsified**, and this project already derives format meaning that way
- obSCEne's tag assignment came from arithmetic holding across every module examined.

Three of the six unknown entries now have something in them, all confirmed 3/3:

- **`0x1` is a digest table.** One SHA-256 per entry in table order, 32 bytes each - 448 bytes
  for a fourteen-entry package, 736 for a twenty-three. Its own slot is zero.
- **`0x100` is the entry table again.** Id at `0x00`, offset at `0x10`, size at `0x14`,
  big-endian, the same fields at the same offsets as the outer table.
- **`0x80` is a digest manifest over named things.** The whole image at `0x40`, `param.sfo` at
  `0xC0`. Three further slots digest something not in the package.

The first two are fully computable and `derive::digest_table` / `derive::entry_table_copy`
produce them. The image digest is computable too.

### What was ruled out, which is most of the work

Killing hypotheses cheaply is the point. Refuted against all three packages:

- the unknown entries are digests of the image *superblock* or a prefix of it
- `0x80`'s remaining slots are digests of any entry
- they are digests of any of the **134 files** extracted from the three packages, by SHA-256 or
  SHA-1
- they are digests of any region of the package file between any pair of interesting offsets -
  header, entry table, first entry, last entry, image start, end of file

Whatever those three slots cover is not in the package. That is worth as much as a hit: it
stops the next person re-running the same searches.

### It ships with the command that re-runs it

`selfish derive <package>...` re-checks every derived claim against packages the reader
supplies and exits non-zero if one fails. `Finding::survived` demands every testable sample
agree, not a majority. The rows in `data/pkg-format.tsv` say `DERIVED` or `PARTLY DERIVED` and
name the command.

