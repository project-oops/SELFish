# D047 - A licence is built from scratch, and a real one is reproduced byte for byte


`licence::Licence::build` produces a RIF from a content id: header, times, the digest-derived
secret IV, the encrypted secret, and a 2048-bit RSA signature. `examples/rebuild_licence`
rebuilds each real package's licence and compares:

```
PS5_ITEM00001_v1.14.pkg: REBUILT BYTE FOR BYTE
PS5_LAPY20011_v1.05.pkg: REBUILT BYTE FOR BYTE
Store-R2-PS5.pkg:        REBUILT BYTE FOR BYTE
```

**There is no partial credit in that test.** A wrong offset, constant, derivation, padding
scheme or key produces different bytes.

### Getting the key was the hard part, and not for the obvious reason

The debug RIF keyset and the AES key over the secret are *published*, in the same project this
repository already takes its fake keyset from. The obstacle was transport: a summarising reader
returned **519 hex characters for a 512-character value**, then overlapping byte ranges, then
sixteen bytes where sixty-four were asked for. Four reconstructions were attempted and all four
were rejected - by signing a real licence and comparing, which is the only test that
distinguishes a nearly-right key from a right one.

The repository was cloned and the file read directly. First attempt, three signatures
reproduced. **A key is not something to obtain approximately.**

### Two things the bytes corrected

- The published field list places the disc key at `0x1E8` and the signature at `0x2A8`. Real
  licences say `0x240` and `0x300`, and only the measured layout ends the signature exactly on
  the structure's boundary. (D046)
- The routine encrypting the secret is named `AesCbcCfb128Encrypt` and sets `CipherMode.CBC`.
  **The name says CFB; the code says CBC**, and a CFB implementation reproduced every other
  field of a real licence and got those 144 bytes wrong. The bytes settled it in one run.

### What this changes about principle 6

The charter said signing "is not attempted and could not be". That is now false and has been
corrected rather than left standing.

Containers still declare themselves fake with zero signature areas - no vendor signature is
forged and none could be. But a licence *is* signed, under a published debug keypair whose
whole purpose is the fake licences a non-retail package carries. Signing with it asserts "this
is a debug licence", which is true, and the licence says so in its own type field.

The line is not "never compute a signature". It is **never claim to be the vendor**.

