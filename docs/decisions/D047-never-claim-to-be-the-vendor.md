# D047 - Never claim to be the vendor

**Status:** decided
**Date:** 2026-09-26

Containers declare themselves fake in the format's own field and their signature areas are
zero. A package licence is built from scratch and signed with the published debug RIF keyset,
and declares itself a debug licence in its type field.

**Why:** signing with the debug keyset asserts "this is a debug licence", which is true. The line
is not "never compute a signature"; it is never to claim to be the vendor.

**Rejected:**
- No licence signature: the package lacks a structure the hardware reads.
- Any signature or field asserting a vendor-signed artefact: a false claim of origin.
