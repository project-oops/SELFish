# D080 - The keystone is derived, not supplied, and every package this crate built was missing one


**The keystone is derived, not supplied, and every package this crate built was missing one.**

Every real package carries `sce_sys/keystone` - 96 bytes - and a builder was demanding it as an
input because it looked like one more file a caller owns. It is not. It is two `HMAC-SHA256`
operations over the **passcode**, which the builder already has:

```text
header      "keystone" 02 00 01 00, then zeros, to 32 bytes
fingerprint HMAC-SHA256(keystone_hmac_key, passcode)
final       HMAC-SHA256(keystone_mac_data, header || fingerprint)
```

Nothing about it identifies a title, so `selfish-pkg::keystone` builds it and the tool puts one
into every image it lays out. `examples/keystone.rs` opens the filesystem inside a package,
extracts the keystone and compares: **96 of 96 bytes identical** on both fake-passcode samples.
The third was built with a passcode nobody can recover and is reported as such.

Found by asking why a package this project builds differs from the ones that install - the same
question that produced D054 and D056, and the third time it has found something certain rather
than probable.

