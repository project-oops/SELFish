# A package built here installs, mounts, loads and executes on hardware


Four defects between "the console mounts `/app0`" and `EXEC /app0/eboot.bin`, found in that
order, each one hidden behind the last. All four came out of comparing our eboot against a
launching homebrew one - the oracle this repository's principle 2 describes, used exactly that
way: derive from something citable, then check against reality, and record which rows reality
settled.

1. **The container entry's `memsz` was `p_memsz`** (D075). Right for a `PT_LOAD` and wrong for
   a segment that is never mapped, where `p_memsz` is zero - so the block layer computed zero
   blocks for a segment with bytes in it.
2. **The dynamic table was left in the image** (D076). A console reads it out of the vendor
   segment's tail, unmapped, with no address at all.
3. **The vendor segment had no fingerprint region** (D077), so every table in it sat `0x18`
   bytes lower than the layout calculation expected.
4. **An executable was declaring an export library** (D074), which cost library id zero. Found
   on the way and fixed on its own evidence; it was not what was failing.

### The surprise, and it cost most of the session

`Failed to load SCE_DYNLIBDATA: 5` names a structure, so it was read as a problem with the
dynamic tables. It is not: three lines above it, in the same log, the authentication manager had
already said `_sceSblAuthMgrLoadSelfBlock: sz for b error` and retried the decrypt four times.
No tag had been read when that error was produced.

Work went into comparing tag numbers, library ids, table contents and conventions - all of it
against a structure the loader had never looked at. **An error naming a structure is not
evidence that the structure is wrong.** It can equally be the layer underneath that failed to
fetch it, and the way to tell is to read the whole log rather than its last line.

The same shape appeared again immediately afterwards, and was recognised the second time:
`does not have DT_SCE_SYMTABSZ or DT_SCE_HASHSZ tabs` named two tags that were present, correct,
and a hundred kilobytes from where the loader was looking.

### What the probes bought

Five throwaway examples under `selfish-container/examples` did all the finding: `tags_probe`
(which tags exist), `tag_values` (what the ones we never emit contain), `tag_diff` (values and
bounds, ours beside real), `symbol_names` (the ids the symbol table independently states), and
`container_entries` (every entry decoded beside the program header it describes).

`container_entries` found defects 1 and 2 in one run, by printing four fields side by side.
Everything before it was reasoning about a file nobody had printed.

