# D043 - Naming a field is not the same as being able to write one


The superblock is now readable field by field, and `Superblock` exposes all of it rather than
the four numbers a reader needs - because keeping only those is exactly what left the rest
looking like a wall.

That does **not** mean filesystem writing is done. What remains is real work rather than
missing knowledge: building an inode table, directory entries and a block allocation from a
directory of files, then compressing to PFSC and encrypting under XTS. `LibOrbisPkg/PFS`
carries `PFSBuilder.cs`, `FSTree.cs`, `FlatPathTable.cs` and `PFSCWriter.cs` for exactly those
steps, so the *source* problem is closed and an *effort* problem is what is left.

Recorded so the distinction survives: item 4 moves from "blocked on a source" to "not written
yet", and those are different entries in a backlog.

