# D001 - One repository for the formats

**Status:** decided
**Date:** 2026-09-26

The file formats orbistoun, obSCEne and prosperous read and write live in this repository,
which holds format knowledge and nothing that knows what a consumer is for.

**Why:** the facts - header layouts, offsets, flag bits, magic values - existed in more than one
project and one copy was wrong. A separate repository also lets format work and consumer work
proceed without sharing a working tree.

**Rejected:**
- A copy in each consumer: the facts drift, and a wrong one ships unnoticed.
- The formats inside one consumer: the others depend on its release cycle and its tree.
