# 2026-09-09 - The symbol level agreed, and the question about it found something


Orbistoun built the second half of the differential and ran it: the dynamic symbol count from
each reader's own derivation, every encoded import joined on symbol index, and the relocation
census by type. **All three agree on all 29 modules.**

The count is the part worth noting, because the two readers derive it from genuinely different
tables - orbistoun from `DT_HASH`'s `nchain`, this crate from `symtabsz / syment`. Two
independent readings of one file, agreeing everywhere. That is the strongest statement either
project has about its symbol reading, and neither could have made it alone.

The only systematic difference is the NID byte order, which their test measures rather than
assumes and reports as a *unit* difference - so the headline is `0 with a disagreement, 28
differing only in units`. Three meaning-traps were checked from this crate's source before
anything was compared, which is the four-rounds lesson applied in advance rather than after.

## What the question found

They asked whether `Info::symbol_count` returning `None` should be surfaced. Looking at *why* it
can was more useful than answering: this crate already has two functions answering that question
differently.

`symbol_count` refuses to infer, and its doc says why - a count inferred from a table's extent
silently changes when something else moves. `symbols` makes exactly that inference: with no
`symtabsz` it walks to the end of the segment.

Same shape as D091 and a different resolution, because this time both standards are right where
they sit. D091's line was *a writer must refuse where a reader may guess*. `symbol_count` is a
claim about the file, so `None` is the honest answer to a module that states nothing; `symbols`
is a reader, and one that stopped dead on a missing size field would read nothing where it could
read almost everything. So the divergence is kept and both docs now say it exists, because what
was actually wrong is that a caller comparing them could not tell which answer it held. (D096)

Latent rather than live: `symtabsz` was present on all 29 modules, and this crate's writer always
emits it. The thing to look at if a module without one ever turns up is that `imports` iterates
`symbols`, so trailing bytes get decoded as symbols - nearly all of which fail
`decode_symbol_name` and are skipped. Nearly all is not all.

## The byte order, answered with a pair rather than an opinion

They asked which of the two docs describes the other convention. The useful answer is not whose
doc is wrong but that **the encoded eleven-character form has no byte-order ambiguity and the
`u64` has**, so it is the form to compare on.

The anchor is one published pair, checkable by anyone with SHA-1 and no console:

```
sceKernelLoadStartModule  ->  wzvqT4UqKX8
```

Under this crate's convention that decodes to `0xc33bea4f852a297f`; the other reading of the
same eleven characters is `0x7f292a854fea3bc3`. Whichever a reader produces tells it which
convention it is in, without anybody having to be wrong in a doc comment.

That is the third distinct byte-order or unit confusion between these two repositories in one
day - the NID `u64`, the table offsets against virtual addresses, and now this. All three cost
a round trip and none was a defect. **A shared value needs its unit written next to it, and the
one form both sides already agree on is the one to exchange.**
