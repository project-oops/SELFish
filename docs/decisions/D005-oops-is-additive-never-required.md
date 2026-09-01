# D005 - OOPS is additive, never required


A meta-repository tracking all four projects, so a build can pull one thing and get the whole
system - and so cross-repository concerns have somewhere to live that is not one of the four.

The rule it has to hold to: **every child stays independently buildable and testable alone.**
OOPS gives you the whole-system build, not the only build. The moment a crate here cannot
compile without it, there is a monorepo with extra steps and a circular dependency to explain.

What earns a place: CI over the whole graph, the integration tests that currently have no
home - *does the probe's container actually load in the emulator* is the most valuable test in
the system and cannot be written inside any single repository - the version matrix recording
which commits work together, shared configuration that is already copy-pasted, and
whole-system setup documentation.

What does not: product code, and anything one repository could own by itself.

Status: **assumed** - the mechanism (submodules or a manifest) is deferred until these are
under version control, and the choice does not matter yet provided OOPS consumes the four
rather than containing things they depend on.

