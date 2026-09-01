# D083 - A comment in the keyset file broke the licence, because the reader matched prose


**A comment in the keyset file broke the licence, because the reader matched prose.**

`read_value` found a key with `KEYS_TOML.split_once(name)` - a bare substring match anywhere in
the file. A documentation pass (D066) added a table to the keyset's header naming each key,
including a row containing `rif_secret_key_hex`. That put the string a hundred lines above its
own assignment, so the reader matched the **comment**, took the next quoted text out of it, and
returned a key of the wrong length. Every licence test failed and the package build stopped
with "the licence could not be built".

Nothing about the change looked like code. The file's prose had quietly been part of its parser
the whole time.

A key is only a key where it is assigned: at the start of a line, followed by `=`. Both readers
now require that, so a comment may name any key it likes - which a provenance header
documenting its own contents obviously has to.

