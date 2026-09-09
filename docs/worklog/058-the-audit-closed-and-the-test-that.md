# 2026-09-09 - The audit closed, and the test that makes it cheap next time


Worklog 057 found two wrong claims in `data/` by grepping for mentions of sibling projects. This
finishes the job one layer out - every doc comment in the crates asserting hardware confirmation
or measurement - and the result is a negative one plus a rule that makes the next pass quick.

## The rule

**Circularity only bites where reality *agreed* with us.**

The magic rows failed because the claim was "a real container confirms what this table says", and
the container sampled was produced by the same lineage as the table. Agreement was guaranteed in
advance, so it carried no information.

A measurement that **contradicted** us cannot be an echo of us. `dynlib`'s import-library
attribute is the clean example: this crate wrote `AUTO_EXPORT` (`0x1`), a real title's
twenty-two `DT_SCE_IMPORT_LIB_ATTR` entries all carry `0x9`, and the crate changed to match. It
does not matter what produced that title - the value disagreed with ours, which no amount of
shared lineage arranges.

So the audit is not "check every hardware claim". It is: **check the ones where reality agreed,
and ask what produced the material.** That is a much smaller set and it is the only set that can
fail this way.

## Applied to the rest

- `dynlib` import-library attribute `0x9` - contradicted us. Safe, and it cost a real bug to
  learn, which is the opposite of circular.
- `pfs` superblock offsets and `MAGIC` - the direction runs image → table. Five were measured
  here *before* `LibOrbisPkg` was read and the source then named them; the rest were read out of
  three real images rather than written and then found. Reading is not confirming.
- `param.rs`'s "four measured off hardware" - measured, and the doc refuses to add a fifth
  without a citable meaning. Principle 5 holding on its own.
- `pkg/lib.rs` already carries a self-critique of exactly this failure: a convention "confirmed"
  by finding high-entropy data at an offset, in an encrypted package where almost every offset
  holds high-entropy data. Somebody had seen this shape before and written it down; it did not
  stop the magic rows, because it was written about one offset rather than as a rule.

That last one is the reason to state the rule as a rule. The knowledge was in the repository, in
prose, attached to the instance that produced it - the same failure as the round-trip warning
living in a test comment (D093) and the explanation living in a `Display` nobody printed (D095).
Three of those in one day suggests **a lesson attached only to its own instance is a lesson that
does not transfer.**

## Where the audit stops

At claims this repository makes. Whether a genuine current-generation eboot carries the current
magic is filed with obSCEne (REQ-20260909T2020Z-8a17) and the row stays marked under review until
it answers. An unresolved row that says it is unresolved is a fine resting place; the problem was
never an open question, it was a closed-looking one that was not.
