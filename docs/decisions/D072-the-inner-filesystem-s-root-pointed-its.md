# D072 - The inner filesystem's root pointed its parent at the super root


`outer.rs` already states the rule, in a comment written when the outer filesystem was built:

> The root's parent is itself. Pointing it at the super root would be the obvious guess and is
> not what a real image does.

The **inner** builder did the obvious thing. `serialise_dir` was handed `super_root` as the root's
parent, so `uroot`'s `..` named inode 0, while three real packages name inode 2 - the root itself.

So one package carried two filesystems that disagreed about a structure they share, and the half
that was right had the reasoning written above it. That is worth more than the bug: a rule
discovered while building one thing is not automatically applied to the other, and nothing in this
repository was checking that the two agreed.

**Why it could matter more than a wrong number usually does.** The super root is internal - it
holds the path table and the root and nothing a title should reach. A consumer walking `..` from
the mount point left the tree instead of staying at its top, and what it found there was a
directory whose entries point back down into the filesystem. This is the shape of fault that does
not return an error.

Whether it is *the* fault is not established. A console panics while mounting the inner image of
every package this crate builds, and this is a real difference from real material found in the
structure that mount reads - which is a reason to fix it, not a reason to announce a cause.

Found with `examples/dirent_probe`, which steps a directory block by the sizes its entries declare
rather than through this crate's reader. The reader is not evidence here: it reads what this crate
writes and it reads real packages, so it is lenient about exactly the thing in question. There is
now a test that reads the bytes for the same reason.

