# Package writing, built up to the wall


`selfish-pkg::write` assembles a package: header, entry table, and every entry this repository
can account for. `selfish pack --image X -o Y` drives it.

The design carries the principle rather than describing it. A build without `0x400`, `0x401`
and `0x1002` **fails naming each one**; a caller supplying `0x1` or `0x100` is refused rather
than allowed to override a computed entry, because two sources for one entry is how a digest
table stops matching what it describes; and every region left blank is reported on the output
with its offset and length, so a hole is something you read rather than something a console
finds.

The check that matters is a test, not an argument: `selfish derive` runs against a package
`write` produced and every claim it re-derives from *real* packages holds. Writer and
derivation agree, and if they ever stop the suite says so.

### The wall, described precisely

The three remaining entries resisted everything local. Neither RSA key from the fake keyset
unwraps any of their 256-byte blocks. None contains the digest of any entry, any region of the
package file, the image or any prefix of it, or any of the 134 files extracted from the three
packages - SHA-256 and SHA-1 both. The same treatment killed every hypothesis for the two
unexplained superblock fields, including HMAC keyed by the seed.

All of that is recorded in D038, and the negatives are the point: **the content is not in the
package**, so no amount of further staring produces it.

### And the source has been under our nose

`data/pkg-keys.toml` has said all along that the fake keyset comes from **LibOrbisPkg**, an
open-source *packaging tool*. A writer is what unblocked the container; this repository already
trusts that project enough to take its keys; and its source is simply not in the local kit.
That is the next move, and it is one fetch away rather than one insight away.

