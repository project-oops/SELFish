# D055 - The passcode reaches further than the key blobs, and a test keyed a package differently to find out


The filesystem key, both key blobs, and the AES over the encrypted entries all derive from the
passcode. `encrypt_licences` had it hardcoded to the fake one, so a package built with any
other passcode had its entries keyed one way and its blobs pointing another - unopenable by a
console *or* by this crate's own reader.

Nothing would have caught it, because every other test uses the fake passcode. The test that
found it builds a package with a different one and reads the key back out the long way.

