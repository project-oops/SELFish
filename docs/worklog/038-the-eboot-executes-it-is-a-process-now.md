# The eboot executes. It is a process now, and it dies on a syscall.


A console creates the process, gives it the display, and runs it:

```text
[Syscore App] new pid=0xb3
[AvControl] VideoOut: shared (pid=0xb3 appId=0x18)
[AvControl] video owner is switched to registered app(appId=0x18)
[Syscore App] Ready to exec
pid 179, uid 0: exited on signal 12
```

**Signal 12 is `SIGSYS`** - a bad or blocked system call. That is a program running and being
stopped, not a file being refused, and it is the first time anything this repository built has
been a process on this machine with a display attached to it.

Three refusals were cleared to get here, each named by the one before it, and the last is the one
worth recording because the error code moved without the message changing:

```text
errno=8    ENOEXEC  - rtld refused the format outright
errno=106           - past validation; the loader accepted the executable
```

`rtld` says nothing at all now, where it used to name `e_type` and then the header range.

### What did it, and what did not

The fix that mattered last was **the headers being inside the first segment**. `module.ld` puts
them there with `FILEHDR PHDRS`, correctly, for a loader that maps segments and then reads the
file through the mapping. A system loader reads the file directly and refuses a first segment
starting at file offset zero. `link/eboot.ld` drops them, and the first `PT_LOAD` now begins at
`0x190` with the header region covered by nothing.

Two things this was *not*, both worth writing down because both were the obvious next guess:

- **Not the import count.** The full probe declares 352 libraries against a real eboot's nine, and
  a minimal build declaring **one** fails identically, same `errno`. Import resolution is not
  reached.
- **Not two linker scripts being incompatible.** Eleven `unable to place section` errors said the
  script was wrong; the actual cause was `-T` being passed **twice** - once from the shared flags
  and once for the eboot - and lld applying both. `TARGET_LD` is now a per-target variable and
  there is exactly one script on any link. An error that names a file is not evidence about that
  file's contents.

### Where it stands

`SIGSYS` at exec is a different investigation from everything before it: the package installs,
the filesystem mounts, the loader accepts the executable and starts it. What remains is what a
title is *permitted to do*, which is the SELF's authentication fields rather than any layout -
`ptype` says fake, and nothing here has yet compared the auth id against a real eboot's.

