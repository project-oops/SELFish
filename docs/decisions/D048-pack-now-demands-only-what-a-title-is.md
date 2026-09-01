# D048 - `pack` now demands only what a title is, not what a package is


Six entries are computed: the two digest tables, the digest manifest, the block-digest table,
and both licences - encrypted, with the entry flags a reader needs in order to know they are.

What remains required is `param.sfo`, the icon, the playgo data, the name table and the key
blobs. **None of those is a format gap.** They are the title's own content and the key material
that belongs to whoever is packaging it; a format library that invented them would be inventing
the title.

A test builds a package, decrypts its licence back out, and verifies the signature - so the
flags, the key index, the entry row and the derivation are all checked together rather than
one console-refusal at a time.

