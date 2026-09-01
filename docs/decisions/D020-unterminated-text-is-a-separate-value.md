# D020 - Unterminated text is a separate value variant, not a flag


`0x0004` is UTF-8 without a terminator and `0x0204` is UTF-8 with one. The first version held
both as `Value::Text` and derived the format from the variant, so every string was written
back terminated.

That turned a PS3 save's unterminated field into a terminated one: a file one byte longer,
differing in a format code fifty-five bytes in. Found only because real files were compared
byte for byte rather than parsed and eyeballed.

Two variants rather than one plus a flag, so a value cannot be held next to a format it
disagrees with. The format is derived from the value and the derivation is now total.

