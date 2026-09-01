# D044 - The licence entries were never unknown - a thirty-two-byte record, not three fields


**The licence entries were never unknown. This crate was reading three fields out of a
thirty-two-byte record and calling the rest padding.**

A package entry record carries more than an id, an offset and a size:

| offset | field |
|---|---|
| `0x00` | id |
| `0x04` | name-table offset |
| `0x08` | flags - **bit 31 marks the entry encrypted** |
| `0x0C` | flags - **bits 12-15 name the key** |
| `0x10` | data offset |
| `0x14` | data size |

`0x400` and `0x401` declare themselves encrypted in every package examined, under key indices 3
and 2. **The image key declares index 3 - the entry this crate has decrypted successfully all
along.** The material was one flags word away the whole time.

Decrypted, `LICENSE_DAT` matches the documented structure field for field:

```
52 49 46 00              "RIF\0"
00 01                    version 1
ff ff                    unknown = -1
00 00 00 00 51 50 61 43  start time = 1364222275
7f ff ff ff ff ff ff ff  end time = i64::MAX
49 56 30 30 ...          content id
```

That start time is the exact constant the source constructor sets. `LICENSE_INFO` decrypts to
the content id followed by zeros.

### Two routes, and they are not equivalent

- **The key blob.** One RSA block inside entry `0x10`, unwrapped with the committed keypair.
  Needs no passcode and works on any package, but carries only the index the image key uses.
- **Computed.** `SHA-256` of the index digest, the content-id digest and the passcode, per
  `LibOrbisPkg/Util/Crypto.cs`. Reaches every index, but only with the passcode the package was
  built with.

`decrypt_entry` tries the blob first because it needs no guess. Where both reach the same entry
they produce **identical bytes**, which `examples/decrypt` checks - and that check is only
performed for the one index where the two are genuinely independent. Comparing a function with
itself and calling it agreement is worse than not checking.

**Of three packages, two use the community default passcode and one does not.** Store declares
a different one; the tool says so rather than presenting noise as a result, and the blob route
still opens its licence.

