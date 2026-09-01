# D053 - Signature ordering is the correctness argument, so it is written as one sequence


Signatures are nested: an indirect block's *contents* are the data blocks' signatures, the
inode blocks' signatures live in an inode embedded in the header, and the header's own digest
covers the region those signatures sit in. So the order is forced - data blocks, then the
indirect block, then the inode blocks, then the header last.

`build` is therefore one long function with an `allow(too_many_lines)` and a reason. Splitting
it by section would hide the ordering, which is the only thing making it correct.

Two limits are refused rather than guessed. A payload needing a **doubly-indirect** signature
block returns an error, because that layout has never been checked here and a plausible-looking
wrong offset costs more than an honest refusal (principle 5). The header's digest is computed
over a range that contains the digest slot, so a verifier has to zero it before recomputing;
that is what the source does, and it is reproduced rather than tidied.

