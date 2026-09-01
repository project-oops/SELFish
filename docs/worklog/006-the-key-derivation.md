# The key derivation


`selfish-pkg` recovers a package's filesystem key. 50 tests across the workspace, clippy clean
under `-D warnings`, fmt clean.

It worked on all three real packages first try:

```text
PS5_ITEM00001_v1.14.pkg   filesystem key: 32 bytes, begins 1d47e1866b11215a
PS5_LAPY20011_v1.05.pkg   filesystem key: 32 bytes, begins eaec6aa3385859ca
Store-R2-PS5.pkg          filesystem key: 32 bytes, begins ca8800700c5149f9
```

**Surprise, and a welcome one: the crypto chain did not change between generations.** Every
source describing it is previous-generation, and it opens current-generation packages
unmodified. The container's magic *did* change, so there was no reason to expect this one had
not - it is a positive result rather than an absence of trouble. (D009)

### The near-miss worth recording

The derivation hashes the image-key entry's **table row**, not the data that row points at.
Two thirty-two-byte quantities attached to one entry. Picking the wrong one yields a key
exactly as plausible and entirely wrong, and nothing between there and an unreadable
filesystem would have said which step was at fault.

`entry_row` is a separate accessor for that reason alone. A single "give me the entry's bytes"
method would have made the wrong choice the natural one.

### The keyset is committed, with what it cannot do written beside it

`data/pkg-keys.toml` carries the public fake-package keyset, its origin, and the sentence that
matters most: these do not open a retail package, because a retail image key is encrypted
under a key nobody outside the vendor has. That is recorded as a wall rather than as
unfinished work, so nobody spends an afternoon trying to climb it.

