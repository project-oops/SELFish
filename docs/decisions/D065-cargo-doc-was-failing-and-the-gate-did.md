# D065 - `cargo doc` was failing, and the gate did not build docs


An audit ran `cargo doc` with `-D rustdoc::broken_intra_doc_links` and found `selfish-pfs`
would not document at all: a link written as `[write]` is ambiguous between the module and the
`write!` macro, and rustdoc treats that as an **error** rather than a warning. Two further
links pointed at private items.

Nobody noticed because **the gate never built the documentation**. In a repository whose
durable product is explicitly its prose - a decision log longer than most of its modules,
provenance notes on every table, reasoning attached to every constant - the deliverable most
worth checking was the one nothing checked.

`scripts/gate.sh` and CI now build docs with three lints denied. `broken_intra_doc_links` is
the one that matters; the other two catch prose that renders wrong.

**And the step was proved to fail before being trusted.** A link was deliberately broken, the
gate was run and returned 1, the link was restored and it returned 0. That is not ceremony:
this script exists because the gate silently swallowed failures twice, and adding an unchecked
step to it would have been the same mistake in a new place.

