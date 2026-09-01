# D041 - The licence entries are stored encrypted, so naming them was not enough


`LICENSE_DAT` is a RIF structure, 1024 bytes, big-endian, with fields the source states
plainly. `LICENSE_INFO` is padded to `0x200`. Both sizes match every sample exactly.

Neither is in that form in a real package. Entry `0x400` should open `52 49 46 00` and does
not - it opens with near-uniform random and contains **one zero byte in 1024**, where the
plaintext structure has a 32-byte zero-filled disc key alone. They are encrypted at rest.

So they stay caller-supplied. What changed is that the gap is now *named and structured*
rather than opaque: a future step knows it is building a RIF, what goes in it, and that the
remaining question is which key wraps it - rather than staring at 1024 bytes of entropy.

The writer requires them and refuses to invent them, which is unchanged and correct.

