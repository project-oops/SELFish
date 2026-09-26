# D090 - An unterminated SFO value is a length, not text

**Status:** decided
**Date:** 2026-09-26

A `utf8_special` (`0x0004`) value is its exact bytes: `Value::TextUnterminated` when they decode
as UTF-8, `Value::Binary` otherwise, never trimmed. `Value::as_bytes` and `Sfo::bytes(key)`
return the bytes whatever the variant. Which end of an id is significant is the caller's
decision.

**Why:** the format carries non-text values such as `ACCOUNT_ID`, eight bytes that may end in
zero. Reading it as text refused whole files over one key and dropped a trailing byte. Some
binary ids decode as UTF-8 by chance, so matching on a variant is unreliable.

**Rejected:**
- Refusing the file on invalid UTF-8: a reader fails over a key nobody asked for.
- Interpreting ids as numbers here: an endianness choice the consumer should make.
