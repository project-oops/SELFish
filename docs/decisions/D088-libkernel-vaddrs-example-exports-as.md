# D088 - `libkernel_vaddrs` example: exports as `name vaddr`, and why the names are not ours

*Renumbered from D086.* Two entries were written as D086 by two sessions; the
on-console audit kept the number, because obSCEne cites it from three source files and
D085 records itself superseded by it. This one had a single citation, in SELFish's own
worklog, and moved.

A consumer (orbistoun, through obSCEne) needs the vaddr of every function a vendor module
exports, to resolve a title's imports against the real library. `dynamic::symbols` already
carries it - a section index and a value per symbol - so
`crates/selfish-container/examples/libkernel_vaddrs.rs` reads a module and prints one line per
**defined** export (`section != 0`, `value != 0`). It is the format-reading primitive, held to a
probe's standards like `symbol_names`, and it is what proves the vaddr measurement independent of
any name.

What it deliberately does **not** do is turn the NID into a name. That needs the mined corpus,
which is obSCEne's measurement product and does not live here (the admission test; the corpus
ruling in this file). So the example emits the encoded NID, and both the name resolution and the
committed `name vaddr` table a consumer reads are `obscene-tool vaddrs`, over there (obSCEne
D265). §2 holds throughout: the example is run against a locally-held `.sprx` that is never
committed, and its resolved output is committed elsewhere under a header naming what it came
from. Status: **done**.

