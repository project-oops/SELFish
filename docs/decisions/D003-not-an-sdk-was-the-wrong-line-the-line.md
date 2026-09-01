# D003 - "Not an SDK" was the wrong line. The line is knowledge against runtime


The charter said this was not an SDK. Questioned, that did not survive: obSCEne compiles for
the console with no vendor toolchain in sight - clang, `lld`, its own linker script, its own
platform declarations, its own format tooling. Between the three projects there **is** an SDK,
and pretending otherwise was guarding a real risk with a slogan.

The risk being guarded is a repository that accumulates a runtime and becomes the place
everything lands. So the line is drawn where the risk actually is:

> selfish holds what you need to **know**. It does not hold what you need to **link**.

Two things were carelessly excluded by the old wording and are readmitted by the new one:

- **Linker scripts and segment layout.** How to lay out a vendor module is format knowledge by
  any reading, and `link/module.ld` fails no part of the admission test.
- **Platform ABI declarations** - arguably. Several hundred signatures exist in a probe to be
  *called* and in an emulator to be *provided*. One set of facts, two consumers, and nothing
  connecting them: they can disagree about an arity today, which is worse than the magic bug,
  since a wrong arity corrupts a stack and faults somewhere with no relation to the cause.

### Why ABI is not moving yet

The two consumers hold the same facts to different standards, on purpose. A probe leaves out
any signature it is not sure of, because calling a function with a wrong arity destroys the
report's credibility. An emulator would rather have the uncertain ones written down than
absent, because an unimplemented function is a known gap and an unknown one is not.

Both positions are correct for their project, so a shared set has to carry the **provenance
level** alongside each signature rather than a single list. The probe already tags every
declaration with one, so the data supports it - but a move that dropped the tag would force
one project's risk appetite onto the other, silently.

Status: **assumed** for the boundary; ABI deferred with the condition above recorded.

