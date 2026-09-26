# D084 - A real container is audited, not transcribed

**Status:** decided
**Date:** 2026-09-26

`selfish audit` compares a real container's fixed header rows, and the four `ex_info` tail rows
the table pins, against `data/self-format.tsv`. It prints what kind of container it read before
the row count, reports the tail apart from the header, and reports each difference with the
real value beside the table's without interpreting it.

**Why:** the table comes from previous-generation sources, and a real file confirms or refutes
a row but cannot define one. A container written from this table matches it whatever it is, so
the kind has to be read first. Tail rows describe fake containers, so a tail difference usually
means "not fake" rather than a wrong layout.

**Rejected:**
- Recording a real file's values as new rows: derives format from material.
- One combined count: mixes a claim about the format with a claim about fakeness.
