# D034 - Two package entries were derived rather than cited, and there is a command that re-derives them


D033 said the six unknown entries all vary between titles and concluded the block was firm.
That was the right measurement and the wrong conclusion drawn from it. "Varies between titles"
does not mean "cannot be computed" - it means "is not a constant", and the two are different.

Challenged on it, the honest position turned out to be that **this project already derives
format meaning from material** where the derivation is arithmetic a machine can check:
obSCEne's vendor tag assignment came from adjacencies like `RELA + RELASZ == HASH` holding
across every module, and D008 settled a current-generation tag by counting. Principle 5 forbids
*inventing* a meaning nothing tests. It does not forbid stating a falsifiable hypothesis and
running it.

Two survived every sample:

- **Entry `0x1` is a digest table.** One SHA-256 per entry, in entry-table order, sized to
  exactly 32 bytes per entry - 448 bytes for a fourteen-entry package and 736 for a
  twenty-three. Its own slot is all zero, because an entry cannot contain its own digest.
- **Entry `0x100` is the entry table again.** Id at `0x00`, offset at `0x10`, size at `0x14`,
  big-endian - the same three fields at the same three offsets the outer table uses.

Both **computable**, which is why they were worth chasing. A writer produces them from data it
already holds, and `derive::digest_table` and `derive::entry_table_copy` do exactly that.

Four remain unknown - `0x80`, `0x400`, `0x401`, `0x1002` - and are not guessed at. `0x80` is
partly digest-shaped (one of its twelve slots is the digest of `param.sfo`); the other three
are dense and high-entropy in every sample.

