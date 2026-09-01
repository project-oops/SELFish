# D075 - A container entry's `memsz` is its data's size, not the segment's memory size


Status: **decided**, 2026-08-29.

The entry field this crate calls `memsz` had been written from `p_memsz`. For an ordinary
`PT_LOAD` the two usually agree, which is why it survived.

They differ twice in a real executable, and both settle it the same way: a `PT_LOAD` with
`p_filesz 0x130` and `p_memsz 0x240` has an entry of `0x130`, and the `PT_SCE_DYNLIBDATA`
segment - whose `p_memsz` is **zero**, because it is never mapped - has an entry of `0x3760`,
its full size on disk. The field is the size of the data the entry describes. Nothing here is
compressed, so it is `filesz`.

The zero is what made this fatal rather than untidy. The authentication manager divides the
field by the block size to decide how many blocks to load, so our vendor segment - `0xb0` bytes
of real content - came out as **zero blocks**:

```text
ERROR: _sceSblAuthMgrLoadSelfBlock(267) sz for b error
[KERNEL] WARNING: Decrypt error in SELF block.  Retrying...(1)  ... (4)
[KERNEL] 661:A: failed to load block
[rtld] ERROR allocate_per_file_info_compact:8016: Failed to load SCE_DYNLIBDATA: 5
```

**The top line is the one that matters and it was the one being ignored.** `Failed to load
SCE_DYNLIBDATA` names a structure, so it read as a problem with the dynamic tables, and a long
stretch of work went into comparing tag numbers, library ids and table contents. Nothing had
read a tag at that point: the loader never got the bytes. An error naming a structure is not
evidence that the structure is wrong - it can equally be the layer that failed to fetch it.

