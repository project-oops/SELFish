# D009 - The crypto chain is unchanged between generations, and that is a finding


Recovering a package's filesystem key takes four steps and three primitives:

```text
entry 0x10, bytes 0x400..0x500  --RSA-2048-->  dk3
sha256(entry 0x20's table ROW || dk3)          -> key and iv
entry 0x20's data       --AES-128-CBC-->       wrapped image key
that                    --RSA-2048-->          the filesystem key
```

Every source for this is previous-generation. **It opens all three current-generation packages
on the first attempt**, each yielding a distinct 32-byte key. So the chain did not change
across the generation boundary - worth recording as a positive result, because the container's
magic *did*, and there was no reason to assume one from the other.

### The row, not the data

The hash takes the image-key entry's **table row** - the thirty-two bytes describing it - and
not the data that row points at. Two different thirty-two-byte quantities associated with one
entry, and choosing wrong produces a key that is exactly as plausible and entirely wrong.

`Package::entry_row` exists as an accessor of its own for that reason. A single method
returning "the entry's bytes" would make the mistake natural and invisible.

### Padding is checked, and this is where a wrong key hides

Both RSA steps yield a PKCS#1 v1.5 block, and the markers are validated before the payload is
taken. Skipping that check does not fail - it produces a key-shaped quantity that decrypts
everything to noise, and the symptom appears hundreds of lines later as an unreadable
filesystem rather than as the wrong key it is.

The failure is also *informative* when it happens: a malformed block almost always means the
package is retail rather than fake, so `NotAFakePackage` says so instead of reporting a
corrupt file. That is a wall rather than a bug, and the error text says which.

### Dependencies, and why not the obvious one

`num-bigint` rather than a full RSA crate. What is needed is one modular exponentiation
against a modulus and exponent already in hand - not key parsing, generation, or signature
verification. A crate doing all four would be a larger surface for a smaller job, and this
code never verifies a signature: it only unwraps blocks somebody else wrapped.

Status: **decided**.

