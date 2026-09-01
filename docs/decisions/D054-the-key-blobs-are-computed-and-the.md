# D054 - The key blobs are computed, and the proof is that they come out byte-identical to real packages


Entries `0x10` and `0x20` carry the key material a console unwraps to reach the filesystem.
This crate used to demand them from the caller, and a caller with nothing to hand supplied
zeros - which produces a package that parses, extracts, and passes every test here, and that a
console cannot open at all. It was the one defect certain to be fatal on hardware.

Producing them needs only the **public** halves of the seven package keys. A public key wraps
and cannot unwrap, so nothing here gained the ability to read anything it could not read
before; what it gained was the ability to write.

The padding is the interesting part. It looks like `PKCS#1` block type 2 but the filler comes
from a Mersenne Twister seeded from the modulus and the payload, so it is **deterministic** -
the right answer is a specific 2048 bytes. That makes the check much stronger than a round
trip: `examples/wrap_keys.rs` compares against real packages and gets **2048 of 2048 and 256 of
256 bytes identical on two independent samples**. A self-consistently wrong implementation
passes a round trip; it cannot pass this.

The seven moduli were extracted from `LibOrbisPkg/Util/Keys.cs` **by a parser, not by hand** -
transcription of key material had already failed four times in this work (D047). The generator
cross-checks its index 3 against the `dk3` modulus already committed, which arrived by a
different route, and refuses to emit unless they are identical. They are.

