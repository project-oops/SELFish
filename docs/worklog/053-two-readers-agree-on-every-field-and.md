# 2026-09-09 - Two readers agree on every field, and the third failure of the day was agreeing


The differential closed. Orbistoun re-ran `selfish-elf` beside its own reader over the same 29
modules and reported **29 modules, 0 disagreements**.

The path there is the entry. Four rounds of reported disagreements, **four harness errors, zero
reader defects**:

1. `Elf::parse` takes an ELF, not a container.
2. `Wrapper::is_either_generation`, not `is_wrapped` - the latter misses `4f 15 3d 1d`.
3. Do not unwrap at all: `Container::to_elf` splices the segment payloads back in. This one was
   ours to give and it took class (2) from 22 refusals to none in a single change.
4. `vendor_tables` ≠ `table.is_some()`, and `dynamic_bytes` ≠ `tables()`. Two pairs of fields
   that sound like the same question and are not.

Only one finding on this side survived: `ProgramHeadersOutOfBounds` was raised from two unrelated
branches and its message described one of them. Orbistoun named the rule better than D095 did -
*a message naming a cause must come from the branch that determined it*.

## The third failure of the day, and it was the cheapest one to avoid

Their report said class (3) "would make this a defect in **this** repository, and it is the one I
am taking away to investigate". SELFish replied: *you are probably right to suspect yourselves*.

That was wrong, and they withdrew it. `vendor_tables` and `table.is_some()` are different
fields - one says where the values came from, the other says which convention the file follows -
and reading the two definitions was the whole answer. Nothing was read. A self-suspicion was
agreed with because agreeing was free.

Three times today, in three different directions:

- a convenient attribution taken on trust, which cost a reversal (D092);
- a pessimistic reading taken on instinct, which nearly cost a false refutation (worklog 051);
- somebody else's self-doubt endorsed without looking at the fields.

The common factor is not optimism or pessimism. **It is answering from what a claim sounds like
rather than from the thing it is about.** Agreeing is not the safe direction; it costs exactly
what disagreeing costs when neither is checked.

## What was answered afterwards, by looking

Orbistoun then asked which of three symbol-level areas to point the next differential at. That
one got checked in the code first, and the answer is worth keeping here:

- **`nid` - settled.** 389 externally-produced pairs constrain the suffix, byte order, alphabet
  and packing at once (D004). A disagreement there is about the decode path, not the hash.
- **`library_id` / `module_id` - least sure, and highest consequence.** They are indices into the
  vendor's own tables rather than `DT_NEEDED`, which this crate's own doc flags as producing
  "attributions which fit and mean nothing" when got wrong - and the area already has a defect on
  record here, found by obSCEne's migration, where a dropped module-version exception bound the
  wrong display library and drew a black window.
- **Symbol count - compare it first regardless**, because everything else is compared by symbol
  index and a wrong count misaligns the lot. `dynamic::symbols` derives it from `symtabsz`, and
  **`info.hash` is recorded and written but never read** - `nchain` is not consulted anywhere in
  this crate. When `symtabsz` is absent it counts to the end of the segment, which over-counts by
  whatever trails the table. Two readers, two different tables, no cross-check: that is the
  comparison worth having.

That last one was not known before this session; it came from following the question into the
code instead of recalling an answer, which is the same habit the three failures above are the
absence of.
