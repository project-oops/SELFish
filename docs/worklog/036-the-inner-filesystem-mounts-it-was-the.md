# The inner filesystem mounts. It was the root's parent all along.


`/app0` mounts. A console takes the package, mounts the outer image, mounts the inner one,
formats the title's download partition, prepares the launch directory and spawns the process:

```text
[PFS] mount finished 1 1 1 1 0 1 0 0        the outer image
[PFS] mount finished 1 0 0 1 0 1 0 0        the inner image - /app0
sceFsUfsMkfsFormatPartition: 256MiB ...     the title's own download partition
MountDownloadData() title_id = [OBSC00001]
PrepareProcessLaunchDir() returned
spawnApp -> createApp OBSC00001
[AvControl] onAppLaunch(appid=0x4018)
```

**No panic, and the console stayed up.** Every crash before this was the mount walking into a
directory whose `..` named the super root instead of the root itself (D072) - an internal
directory holding the path table and the root, whose entries point back down into the filesystem.
One wrong inode number in one directory entry, and the machine went down rather than returning an
error, four times.

Worth stating plainly because the search took a long route: the fault was in the *first* structure
a mount reads, it had been there the whole time, and the rule it broke was written down in this
repository, in `outer.rs`, above the line that gets it right.

### What the launch fails on now, which is a different problem

```text
[rtld] ERROR _exec_self_imgact:1375: Unsupported ELF e_type. /app0/eboot.bin fe10
```

The system loader reads the eboot and refuses it. Measured against the eboot inside a real
package, two things differ and both are already known here:

| | this build | a real package's eboot |
|---|---|---|
| container magic | `54 14 F5 EE` (current) | `4F 15 3D 1D` (**previous**) |
| `e_type` | `0xFE10`, executable | `0xFE00`, **fixed-address** executable |

The container half is not news - `selfish wrap` already defaults to the previous generation
because thirty-three containers inside real current-generation packages carry that magic, and the
Makefile driving this build passes `GEN=5` anyway. The `e_type` half is new: a module built to be
loaded by an emulator or by `elfldr` is `0xFE10`, and the thing a *system* loader executes from
`/app0` is `0xFE00`.

So the eboot in a package is not the module this project has been building all along, and that is
a container question rather than a filesystem one. The important part is that it is a clean
refusal from a running console with a mounted filesystem, which is a different kind of problem
from a machine that stops answering.

