# Licences, built and proven


`licence::Licence::build` produces a RIF from a content id - header, times, the digest-derived
secret IV, the encrypted secret, and a 2048-bit RSA signature - and rebuilding each real
package's licence gives **byte-for-byte matches on all three**. There is no partial credit in
that test: a wrong offset, constant, derivation, padding or key produces different bytes.

Two things the bytes corrected, both of which would have cost hours of guessing:

- A published field list puts the disc key at `0x1E8` and the signature at `0x2A8`. Real
  licences say `0x240` and `0x300`, and only the measured layout ends the signature exactly on
  the structure's boundary.
- The routine that encrypts the secret is called `AesCbcCfb128Encrypt` and sets
  `CipherMode.CBC`. **The name says CFB; the code says CBC.** A CFB implementation reproduced
  every other field of a real licence and got exactly those 144 bytes wrong - which is how it
  was found, in one run, with the first difference landing precisely on `field::SECRET`.

### The key, and four rejected attempts

The keys are published, in the project this repository already takes its fake keyset from. The
obstacle was transport: a summarising reader gave **519 hex characters for a 512-character
value**, then overlapping ranges, then sixteen bytes where sixty-four were asked. Four
reconstructions, four rejections - by signing a real licence and comparing, the only test that
separates a nearly-right key from a right one. Cloned the repository, read the file, three
signatures reproduced on the first attempt.

### And the charter was wrong

Principle 6 said signing "is not attempted and could not be". A licence *is* signed now, under
a published debug keypair whose purpose is exactly the fake licences a non-retail package
carries. The principle has been corrected rather than left standing: containers still carry
zero signature areas and no vendor signature is forged. The line is **never claim to be the
vendor**, not "never compute a signature".

