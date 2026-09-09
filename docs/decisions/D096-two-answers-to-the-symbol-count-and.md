# D096 - Two answers to the symbol count, and the divergence is kept rather than reconciled

**Status: decided. 2026-09-09.**


**`Info::symbol_count` refuses to infer a count and `dynamic::symbols` infers one. Both are
right, and what was wrong is that neither said so.**

Orbistoun's symbol-level differential asked whether `symbol_count` returning `None` should be
surfaced. Looking at why it can returned something better than an answer: the crate already
contains two functions that answer the same question differently.

- `Info::symbol_count` returns `symtabsz / syment`, and `None` when either is zero. Its doc is
  explicit about why: *"a symbol count inferred from a table's extent is a count that silently
  changes when something else moves"*.
- `dynamic::symbols` makes exactly that inference. With no `symtabsz` it walks to the end of the
  segment, so its length is bounded by the segment rather than by the table, and anything
  trailing the symbols is decoded as more of them.

## Kept, not reconciled

This is the same shape as D091 - two functions, one crate, one question, opposite standards -
and the resolution is different, because this time the two standards are both correct for where
they sit.

D091's line was that **a writer must refuse where a reader may guess**. `symbol_count` is a
claim about the file: it is asked "how many symbols does this module say it has", and the honest
answer to a module that says nothing is `None`. `symbols` is a reader: one that stopped dead on
a missing size field would read nothing at all where it could read almost everything, and a
wrong line on a terminal is the worst its guess can produce.

So the fix is not to make one behave like the other. It is that a caller comparing them had no
way to know which of the two answers it was holding, and now both docs say. A caller that needs
the exact count asks `symbol_count` and treats `None` as *this module does not state one*.

## Why it is not being changed on the evidence available

Latent, not observed: orbistoun's differential found `symtabsz` present on all 29 modules of its
corpus, and this crate's own writer always emits it. Nothing has been seen to take the inferring
path. Principle 5 - the change to make is the one material asks for, and no material has asked
for one.

The one thing worth watching, recorded so it is not rediscovered: `imports` iterates `symbols`,
so on a module without `symtabsz` it would decode whatever trails the symbol table as further
symbols. They would almost all fail `decode_symbol_name` and be skipped, which is why this is a
latent oddity rather than a live defect - but "almost all" is not "all", and a plausible
eleven-character string in trailing data would enter an import list. If a module without
`symtabsz` ever turns up, that is the thing to look at first.
