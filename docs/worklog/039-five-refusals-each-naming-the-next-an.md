# Five refusals, each naming the next: an eboot walked into a console's loader


A console's `rtld` now reads this project's eboot, validates it, accepts every header, and gets as
far as loading its dynamic tables. Each refusal along the way named the next one, and every fix
came from measuring a real package's eboot rather than reasoning about what a loader ought to
want - which was wrong three times out of five.

```text
1  errno=8   ENOEXEC                    Unsupported ELF e_type ... fe10
2  errno=8                              verify_ehdr: B: offset 0x0  end 0x190
3  errno=106 SIGSYS                     no message; process created, display handed over, killed
4  errno=8                              scan_phdr: B: align 16384 va 0x400190 offset 0x190
5  errno=8                              scan_phdr: B: error 8  i 5
6  errno=0                              Failed to load SCE_DYNLIBDATA: 5   <- here now
```

| # | what it was | the fix |
|---|---|---|
| 1 | `e_type` `0xFE10`, what a mapping loader wants | `0xFE00`, fixed-address, via `mkmodule --fixed` |
| 2 | the ELF header inside the first segment | `link/eboot.ld` without `FILEHDR PHDRS` |
| 3 | `EI_ABIVERSION` 2 and current tables in a previous-generation container | previous generation throughout, `EBOOT_GEN`/`EBOOT_TABLE` |
| 4 | segments merely *congruent* to the page size | based at `0x400000` exactly, so `va` and `offset` are both aligned |
| 5 | `PT_SCE_DYNLIBDATA` with `p_memsz = p_filesz` at address zero | `p_memsz = 0` when the segment is not mapped |

### The two worth keeping

**Number 3 is why an error code moving is not always progress.** A kernel carries three syscall
vectors - previous-generation, FreeBSD, and native - and selects between them from the
executable's identity. This build was emitting a previous-generation *container* around a module
stamped current with current-generation *tables*: three identities in one file. Under the wrong
vector the loader's first syscall is not in the table it selected, the kernel returns `ENOSYS` and
sends `SIGSYS`, and what you see is a process that was created, given the display, and killed with
no message about why. `GEN` still decides what the code targets; what changed is what the file
says it is.

**Number 5 was written down here already.** `repurpose_header` set `p_filesz` and `p_memsz` to the
same value, above a comment explaining that the legacy segment "is never mapped". A segment with
no address and a non-zero memory size asks to be placed at the null page, and `rtld` refuses the
file and names the index. Every vendor data segment in a real eboot - `PT_SCE_DYNLIBDATA` and both
`0x6FFFFFxx` - carries a memory size of zero. The code knew; only the two `put_u64` calls did not.

### Where it stands

```text
[rtld] ERROR allocate_per_file_info_compact:8016: Failed to load SCE_DYNLIBDATA: 5
[rtld] ERROR _exec_self_imgact:1869: dynlib_proc_initialize_step1() returned 5
```

Past every header check and into the loader's own table handling. This crate reads the same tables
back correctly - one import, `libkernel` - so what remains is not a malformed table but something
about how a console's loader wants them presented. The minimal eboot's segment is 176 bytes where
a real one's is 14,176; whether that matters is the next thing to establish rather than assume.

### `SCE_DYNLIBDATA` is not refused for its size or its contents

The obvious next guess was that a 176-byte table segment is too small, or missing something a
larger one would have. It is not: a build whose segment is **3,564,392 bytes** - twenty times the
size of a real eboot's 14,176 - is refused at the same line with the same code.

```text
   176 bytes  Failed to load SCE_DYNLIBDATA: 5
 3.5 Mbytes   Failed to load SCE_DYNLIBDATA: 5
```

So it is structural, and two things are already ruled out with it. The container's entry table is
correct: entries pair a digest with a data record, the digest naming the *table index* it covers
and the data naming the *segment index* it holds, and ours does that for segments 0, 1 and 5 with
the same `0x2804` flag bits a real eboot uses for 0, 1, 2, 8 and 9. And the segment header now
matches a real one field for field - offset, address, `p_memsz` of zero, `0x10` alignment.

What is left is how the tables inside it are shaped. Two candidates, in order:

- **The tag layout.** This crate reads its own tables back correctly, which proves the round trip
  and not the format: `crate::dynamic` and `crate::dynlib` agree with each other by construction.
  A real eboot's tags are the only third opinion available, and they have not been compared.
- **The convention.** The function that refuses it is `allocate_per_file_info_compact`, and this
  build now writes the *legacy* convention because a real eboot does. The one configuration never
  tried is legacy tables at the current generation, or its converse - the two halves have only
  ever moved together, because `TABLE` derives from `GEN`.

Worth noting which way the evidence points before testing either: with current tables the loader
got *past* this point and died later, at the syscall vector. That is not evidence that current is
correct - it is evidence that the two failures are independent, and that this one is reachable
only once the vector is right.

