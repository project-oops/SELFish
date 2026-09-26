# D098 - Measured vendor values are notes on rows

**Status:** decided
**Date:** 2026-09-26

A value measured in genuine vendor containers is recorded in the note of the `data/` row it
confirms or refutes, with the sample and count. It is never a second set of rows or a named
profile.

**Why:** a profile with a value column is something a writer can emit, and a container built
from it would claim to be a signed vendor executable (D047). In the samples, authenticity and
container kind are confounded, so the name "signed vendor" would also assert something the
measurement cannot separate.

**Rejected:**
- A "signed vendor profile" in the format table: reachable by a builder, and misnamed.
