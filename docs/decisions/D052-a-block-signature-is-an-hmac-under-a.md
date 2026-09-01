# D052 - A block signature is an HMAC under a key the builder computes


**A block signature is an HMAC under a key the builder computes, so signing the outer image
needs nothing that has to be obtained.**

This is worth stating next to D047, because the surrounding work spent a long time establishing
that some keys genuinely cannot be recovered. `EKPFS` is not one of them:

```text
EKPFS     = SHA-256(digest(1) || digest(content id padded to 48) || passcode)
sign key  = HMAC-SHA256(EKPFS, LE32(2) || seed)
xts keys  = HMAC-SHA256(EKPFS, LE32(1) || seed)
```

Content id and passcode are both *inputs a builder chooses*. Nothing is recovered, nothing is
forged, and a "signature" here is a keyed digest rather than a claim of authorship.

`selfish-pkg`'s `derive_filesystem_key` computes it; the pre-existing `filesystem_key` recovers
it from a package by decrypting the image-key entry. **The two are an oracle for each other**,
and `examples/filesystem_key.rs` runs it: on the packages to hand, two of three agree exactly,
and the third differs only because it was not built with the fake passcode - proved by opening
its image with the recovered key. All three images open. The derivation a writer depends on is
the one real packages use.

