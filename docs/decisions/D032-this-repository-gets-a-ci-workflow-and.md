# D032 - This repository gets a CI workflow, and obSCEne's is marked as unable to pass


Neither has ever executed - there is no remote - so both are documentation claiming to be
gates. The difference is worth recording because it decided how each was written.

**selfish's can be right**, because this repository has **no path dependencies outside
itself**. That is not luck; it is what being the bottom of the spine means, and it is the one
property to preserve if anything is ever added here.

Two things in it are not obvious:

- **It installs `clang` and `lld`, and then checks the linker tests did not skip.** Those tests
  skip rather than fail without a toolchain (D024), which is right for a developer's machine
  and wrong for CI: a missing linker would leave the job green having verified nothing about
  the segment layout, the tag conventions or the module surgery. A skip-detector turns the
  most valuable tests in the suite from silently optional into required.
- **A provenance job**, because the formats are the product and their provenance is what the
  product is worth. It checks every `data/*.tsv` carries a `read-from:` header, and that no
  material is committed - principle 2 is easier to enforce on the way in than to remove from a
  history later.

**obSCEne's cannot pass as written**, and this migration is half the reason. Its `tool/` takes
path dependencies on two sibling repositories and every job does a single bare checkout, so
neither path exists. The `pros-link` half has been latent since D189; the `selfish` half
arrived with D200.

Flagged in the workflow with the shape of the fix, rather than applied. Restructuring eight
jobs that nothing can execute, against repository names that are not settled, in a repository
another session is actively editing, is a guess wearing the costume of a fix.

