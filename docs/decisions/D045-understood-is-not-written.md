# D045 - Understood is not written


Both licence entries can now be read and both structures are documented. Producing one still
needs work: `LICENSE_DAT` carries a secret derived by hashing and an RSA-2048 signature, and
neither is laid out by this crate yet.

So `write::Builder` still requires them, and that is correct rather than pending. What changed
is the category - from "dense entropy nothing can explain" to "a documented structure whose
builder has not been written". The first is a wall; the second is a task.

