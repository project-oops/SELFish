# The indirect block looked guilty and is not, which sets up the next test


Which packages take a console down correlated exactly with whether they use an indirect signature
block - the twelve-block one survived, the 28- and 171-block ones panicked - and a real package
with 944 blocks mounts, so the format is fine and the writer would have been the obvious suspect.

`examples/indirect_probe` compares the two directly, and they are the same file:

```text
                  inode slots           indirect at   entries   after
  ours   171 blk  0x7..0x12 + 0x5       0x5           159       zeroes
  real   944 blk  0x7..0x12 + 0x5       0x5           932       zeroes
```

Same twelve inline block numbers, same thirteenth slot naming the same indirect block, same
`36`-byte stride inside it, and `blocks - 12` entries in both. There is nothing wrong with it.

**The correlation was an artefact of when each package failed.** The twelve-block package did not
survive because it avoided the indirect block; it survived because it was refused at the cache
check (D070), which happens *before* whatever panics. It never reached the code in question. A
package that fails earlier for an unrelated reason is not a control - it is a package that was
never tested - and reading it as one nearly sent a session into rewriting a correct block writer.

The cache clamp turns it into a real control. That package now passes the check that stopped it,
reaches the mount, and carries no indirect block, so the next hardware run splits the remaining
possibilities cleanly:

- **it panics** - the indirect block is exonerated by experiment as well as by inspection, and
  the fault is somewhere every package reaches
- **it fails cleanly** - something that only large images reach is implicated after all

Either answer is worth a jailbreak cycle, which is not true of most things that could be tried
next, and the package is already built.

