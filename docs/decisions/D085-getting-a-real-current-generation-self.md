# D085 - Getting a real current-generation SELF is a measurement obSCEne runs, not a file this repo holds - and the sandbox makes it conditional


**Getting a real current-generation SELF is a measurement obSCEne runs, not a file this repo
holds - and the sandbox makes it conditional.**

The gap D084 leaves is having a real current-generation container to audit. The answer is the
pattern this project already uses for the import hash: obSCEne is the instrument, its output is
the product, and the vendor material never enters the repository.

**Superseded by D086 - the dump was replaced by an on-console audit that never copies bytes off
the box.** The paragraph below describes the first design, kept for the record.

`obscene/src/sections/selfdump.c` (section `047-selfdump`, **wired and building for both
targets**) reads the first `0x1000` bytes of a real SELF already on the box - the system eboot -
and writes them to a writable path for a person to collect and run through `selfish audit`. It
copies a header out; it does not decrypt, name a field, or decide what a difference means.

It went through obSCEne's own five-step discipline for adding a check and its `make host` rule.
It compiles for host and for the freestanding console module under the full `-Werror
-Wconversion -Wsign-conversion` set, links into both, passes `mkmodule`, and on the host run
announces then **skips cleanly** - "no real SELF was reachable from this process" - which is the
correct result anywhere the system eboot is outside the process's view. The reader half is
proven by composition: a verbatim first-`0x1000`-byte copy of a real gen-5 SELF audits exactly as
the whole file does. It uses the portable `obs_sink_backend_*` file primitives rather than raw
open flags, because the create/truncate flags are target-specific and a section that hardcoded
them would be right on one target and wrong on the other. Still **not run on hardware**: the
skip-or-dump behaviour there is what a real console visit settles.

Two honesty points recorded with it:

- **The sandbox gates it.** A sandboxed module sees its own `/app0` and little else; the system
  eboot is outside that. A payload with the exploit's privileges sees the broader filesystem.
  So the probe *skips* cleanly when the source is unreachable - a skip is itself a measurement
  ("not reachable from where this ran"). "Running under jailbreak" does not by itself free the
  filesystem view: it grants kernel R/W to *some* process, and escaping the jail is an action
  (patch the process's own sandbox) rather than an automatic state. obSCEne has a network escape
  hatch today, not a filesystem one.

- **It was not wired in.** obSCEne was being restructured by another session and console C
  cannot be compiled from where this was written. An unregistered section is inert - not in the
  Makefile, not compiled - so the file is safe to leave in place, and the wiring is recorded in
  its own footer for whoever has the SDK.

