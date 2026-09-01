# Running the finished thing, and finding the last missing field


With the writer done, the obvious next step was to drive the whole chain by hand rather than
through a test: link a module with the script here, build its vendor segment, wrap it in a
container, and read it back.

It came out as **an ordinary shared object with a SysV ABI**. Every unit test passed; the
end-to-end test passed. The reader had been saying so on its own third line the entire time:

```
type       e_type 0x0003
osabi      NOT FreeBSD - a loader refuses this before anything else
```

Three header fields no linker sets, because no linker knows about either console, and a loader
refuses the file on any of them before it reads a single other byte. They were the last piece
of format knowledge still living only in obSCEne. `identity::stamp` now writes them, reporting
each change, and the end-to-end test asserts the module is an executable with the right ABI
before it reads anything else. (D028)

### Two things worth keeping from how it was found

**Unit tests all passed.** Each piece was correct about its own job. What was missing was a
step nobody owned, and only running the thing end to end and *reading the output* surfaced it.

**The reader already knew.** It had a specific, correct diagnostic - written by someone who had
clearly been bitten - and the writing side simply never asked it. Which is an argument for
principle 4 that is easy to state and hard to remember: a reader and a writer in one crate is
worth nothing if only the reader is ever pointed at the output.

