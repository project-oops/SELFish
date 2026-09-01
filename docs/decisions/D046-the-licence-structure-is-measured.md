# D046 - The licence structure is measured; producing one needs two keys this repository does not hold


Entry `0x400` decrypts to a RIF and the layout is now in `data/pkg-format.tsv`, **measured from
three real licences rather than transcribed**. That distinction earned its keep immediately: a
published field list places `DiscKey` at `0x1E8` and `Signature` at `0x2A8`, which cannot be
reconciled with the bytes - the populated region is exactly `0x260..0x400` in every sample, and
a signature at `0x2A8` would end `0x58` short of the structure. The measured offsets put the
signature's last byte exactly on the boundary.

One field is confirmed *derived* rather than merely located: `secret_iv` equals the first
sixteen bytes of `sha256(content id padded to 48 with NULs)` in all three.

### What is actually missing

Not policy, and not knowledge of the format. **Two keys:**

- `rif_debug_key` - sixteen bytes, AES-CFB128 over the `secret` field. Short enough to obtain
  reliably.
- **the debug RIF RSA keyset** - signs `sha256` of the first `0x300` bytes. Neither committed
  keypair produces or verifies a real signature; both were tried, in both directions, with
  `e = 65537`, `e = 3` and the private exponent. This is a third keyset.

Both are published in the same open-source packaging tool this repository already takes its
fake keyset from, so this is a fetch rather than a wall - but not a fetch that can go through a
summarising reader. A 2048-bit key transcribed that way came back **519 hex characters where it
must be 512**, and the verification below caught it rather than a wrong signature reaching a
package.

### The verification is the useful part

Whoever supplies that key gets an immediate yes or no: sign the first `0x300` bytes of a real
decrypted licence and compare with its stored signature. A correct key reproduces it exactly.
There is no need to trust a transcription, and no way for a wrong one to pass quietly.

That check is why this stopped here rather than committing a key that looked plausible.

