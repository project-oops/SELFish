# Producing something the hardware will load

For most workflows, **the pipeline** runs the complete assembly in a single invocation:

```
selfish --input <file> --target <orbis|neo|prospero|trinity> --format <elf|eboot|title|pkg> --output <path>
```

- `--format elf`: stamps the platform identity a loader checks before anything else.
- `--format eboot`: that, wrapped in a signed-executable container.
- `--format title`: container plus `param.json`, `icon0.png`, keystone, laid out as `<output>/<TITLE_ID>/`.
- `--format pkg`: that, plus `param.sfo` and a PFS image, assembled into a `.pkg`.

`--target` carries the generation; nothing passes a generation number.

Underneath, between a compiler and the hardware there are four steps, and then a fork. `selfish`
provides format diagnostic commands for each layer independently - which matters, because when
a file is rejected the useful question is *which step produced the wrong bytes*.

```
   compiler output
        |  stamp     the platform identity no linker sets
        v
      module  ------------------------------------> a payload, and it can stop here
        |  wrap      the signed-executable container
        v
    eboot.bin
        |                              \
        |  image     the filesystem     \  build title   lay out a Prospero title directory
        v            the title mounts    v
   image file                         <TITLE_ID>/
        |  pack      the package          param.json + sce_sys/
        v            around it
   package.pkg
```

**The executable never changes across any of this.** What changes is the wrapper, and each
wrapper decides something different about where the code ends up running: a payload runs
under a homebrew loader, a package installs through the previous generation's compatibility
path, a title directory is a current-generation (Prospero) entry on the home screen.

Every transcript below is real output, from one 35 KB payload run through the whole set.

## 1. Stamp the identity

```console
$ selfish stamp payload.elf --target prospero
  EI_ABIVERSION  0x0 -> 0x2
  e_type         0x3 -> 0xfe10
  p_flags (R+X -> X) 0x5 -> 0x1
3 field(s) written to payload.elf
```

Rewrites the file in place, and says which fields and what they were. No linker sets these,
because no linker knows about either console - so a freshly compiled ELF is not yet a module,
and nothing about it looks wrong until a loader refuses it.

`--library` stamps it as a shared library rather than an executable.

**A stamped file is already a payload.** If a homebrew loader is going to map and run it, the
remaining steps are not needed at all.

## 2. Wrap it in a container

```console
$ selfish wrap payload.elf --out eboot-gen4.bin --target orbis
eboot-gen4.bin: 30368 bytes from a 35720 byte payload, previous generation (privilege: App, sdk: 0x08008011/0x00000000)

$ selfish wrap payload.elf --out eboot-gen5.bin --target prospero --sdk ps5-native
eboot-gen5.bin: 30368 bytes from a 35720 byte payload, current generation (privilege: App, sdk: 0x08050001/0x02000009)
```

Without `--out` it writes `eboot.bin` beside the input.

The generation changes the four bytes at the front:

```console
$ head -c4 eboot-gen4.bin | od -An -tx1
 4f 15 3d 1d
$ head -c4 eboot-gen5.bin | od -An -tx1
 54 14 f5 ee
```

**Do not read those as simply "old" and "new"** - the labelling is subtler than the flag
makes it look, and [the glossary](../GLOSSARY.md#the-generation-split) has it: a real
current-generation *app eboot* carries the first, and a title's *bundled modules* carry the
second. What makes a title native is `param.json` and native registration, not the magic.

**`--target` defaults to `orbis`, and that is a route decision rather than a habit.** Both
generations are accepted by current hardware, each proven on its own delivery route: a
*package* with the previous generation's magic installs, mounts, loads and executes (worklog
040), and a *native title* with the current magic installs, launches and ran 292 checks to
completion (obscene sweep 20260909-184538). Neither has been shown accepted on the other's
route, and the default serves the package path.

The justification used to be a population - "every container inside the real packages sampled
carries the previous generation's magic". **That is withdrawn**: those containers were of
homebrew lineage, and every *genuine* container measured carries the current magic, 23 of 23.
The better population points the other way and is equally not the reason. A default turns on
what a loader accepts. (D097)

**`--sdk` takes an alias or a literal version.** `ps5-native` resolved to
`0x08050001/0x02000009` above; `2.000.009` and `ps4-compat` are equally valid. The dictionary
is `data/sdk-versions.toml`, outside the source, because a version table is data rather than
code, and it is validated against the generation - a current-generation version on a
previous-generation container is refused rather than written. Getting this wrong is not
subtle: the loader refuses the process with an SDK-version error before any of your code
runs.

`--privilege` takes `app`, `sysmodule`, `system` or `root`.

The container declares itself fake in the field the format provides for exactly that, and its
signature area is zero. No vendor signature is forged and none could be.

## 3. Build the filesystem image

```console
$ selfish image --root app --out title.pfs.img --content-id UP0000-OBSC00001_00-0000000000000000
sce_sys/keystone: generated from the passcode
title.pfs.img: 1048576 bytes
keyed to UP0000-OBSC00001_00-0000000000000000 - a package carrying this must declare the same id
```

The files become a plain filesystem, wrapped in a `PFSC` container, carried as the single
file of a signed and encrypted outer filesystem.

**The content id is not optional, and getting it wrong is the quiet failure.** The image is
encrypted under a key derived from the content id and the passcode, so an image built under
one id cannot be opened by a package built under another - and nothing about the resulting
file looks wrong until a console tries to mount it. That last line of output exists to make
the mismatch hard to reach.

`--passcode` defaults to the fake one, which is what you want unless you have a reason.

## 4. Pack it

```console
$ selfish pack --image title.pfs.img --out title.pkg --content-id UP0000-OBSC00001_00-0000000000000000 --title-id OBSC00001 --title "Demo"
warning: the inner filesystem is 589824 bytes. The hardware refuses to mount an image this small - `Failed to enable GDDR5 cache`, EINVAL, after the outer image has already mounted - and lowering the declared cache size does NOT help: it was tried, set to exactly 589824, and the hardware refused it identically. Pad the directory past 851968 bytes.
param.sfo: generated for OBSC00001 ("Demo", version 01.00)
icon0.png: none given, using selfish own - supply --entry 0x1200=FILE to replace
playgo-manifest.xml: generated the default manifest
no contents for entries: 0x200 0x1001 - nothing here can compute them, so they must be supplied
```

**No package was written, and that is the feature.** Everything derivable was computed and
reported; the two entries that cannot be computed from anything this repository knows are
named, and the build stops. A package that assembled itself by guessing an entry is a package
that installs and then fails somewhere with no connection to the guess.

Supply them and the same command completes:

```bash
selfish pack --image title.pfs.img --out title.pkg \
    --content-id UP0000-OBSC00001_00-0000000000000000 \
    --entry 0x200=names.bin \
    --entry 0x1001=playgo-chunk.dat \
    --entry 0x1200=icon0.png
```

The size warning is a measured fact rather than a guess: the threshold, the error string and
the workaround that *did not* work were all established against hardware.

`--dir` runs the whole chain - step 3 and step 4 together - which is the usual way in:

```bash
selfish pack --dir app --out title.pkg --content-id UP0000-OBSC00001_00-0000000000000000
```

`--image` takes an image you already built, for when you are iterating on the package around
a filesystem that has not changed.

A package's licence carries a signature under the published **debug** keyset, whose entire
purpose is the fake licences a non-retail package holds. Signing with it asserts "this is a
debug licence", which is true, and the licence says so in its own type field. The line is not
"never compute a signature" - it is never claim to be the vendor.

**A package installs as a previous-generation title**, which is why homebrew shipped this way
runs in the compatibility sandbox. That is what the install path is, not a limitation of this
repository. obSCEne's D255 is the finding.

## 4b. Or lay out a title directory instead

The current generation's (Prospero) own format is not a package at all, and this is the other half of
the fork. It is a directory described by `param.json`, registered by a call that needs kernel
privileges:

```console
$ selfish --input payload.elf --target prospero --format title --title-id OBSC00001 --title "Demo" --output native
1 file(s) copied from app
privilege: App
contentId: UP0000-OBSC00001_00-0000000000000000
native/OBSC00001/sce_sys/param.json
native/OBSC00001/sce_sys/icon0.png (generated)
native/OBSC00001/sce_sys/keystone (generated)
native/OBSC00001/sce_sys/pfs-version.dat (generated)
native/OBSC00001/sce_sys/nptitle.dat (generated)

install by copying native/OBSC00001 to /user/app/ on the
target and calling sceAppInstUtilAppInstallTitleDir("OBSC00001", "/user/app/", 0).
that call needs kernel privileges, so it runs from a payload rather than from here.
```


Four of the five metadata files are generated because they are derivable; the eboot came in
through `--root`, which copies extra files verbatim (and wraps `eboot.elf` into `eboot.bin` automatically
if it has not already been wrapped). This lays out bytes and then says what it cannot do - the registration
call is not one a host-side program can make, so it names the call rather than appearing to install anything.

`--deeplink` makes an entry that launches something else instead of carrying its own
executable, which is the shape to use when the code is already running as a payload.
`--category` and `--privilege` default to what a Prospero homebrew entry uses (`category: 65536`, `privilege: app`).

### Choosing `--category` and `--privilege`

Execution on PlayStation systems is governed by two orthogonal axes:

1. **Privilege Tier (`paid` / Authority ID in the SELF header)**:
   - `app` (`0x3800000000000000`): Standard sandboxed execution.
   - `sysmodule` (`0x3800000000000001`): Dynamic system module privileges.
   - `system` (`0x3800000000000001`): Unsandboxed system service privileges.
   - `root` (`0x8000000000000001`): Full kernel root authority (jailbreak syscalls, arbitrary `/dev/*` nodes).

2. **Application Category (`applicationCategoryType` in `param.json`)**:
   Controls Direct Memory (DMEM) hardware allocation, HDMI video scanout ownership (`SceSysAvControl`), and process lifecycle:

| Category Value | Constant / Name | Memory (DMEM) | HDMI Video Out | Multi-Tasking Behavior | Requirements / Gotchas |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`0`** (`0x00000000`) | **`BIG_APP`** (`native_game`) | **Full Budget**<br>~12.5 GB (PS5)<br>~5.5 GB (PS4) | **Exclusive Primary**<br>(Bus 0 / `OBS_VIDEO_BUS_MAIN`)<br>Full 4K / HDR / 120Hz | **Foreground exclusive**.<br>Only one Big App can run at a time; launching another suspends or closes the current one. | Requires `/app0/sce_module/libc.prx`<br>Gated by `PFAuthClient` (`pltauth-patch` required) |
| **`65536`** (`0x00010000`) | **`SYSTEM_APP`** | **0 Direct Memory**<br>(Userland `mmap`/`malloc` only) | **Denied Primary**<br>(`sceVideoOutOpen` fails `0x80290001`) | Background utility or standalone daemon | No `libc.prx` or `pltauth` required |
| **`131072`** (`0x00020000`) | **`MINI_APP`** | **Restricted Budget**<br>(~256 MB – 512 MB) | **Overlay Compositor**<br>Renders to compositor layer, not raw HDMI | **Concurrent**.<br>Runs simultaneously alongside a running Big App (e.g. Spotify, Quick Menu) | Constrained PRX & system limits |
| **`3`** (`0x00000003`) | **`DAEMON`** | System memory pool | **None**<br>(Headless only) | Background system services | Headless execution |
| **`262144`** (`0x00040000`) | **`MEDIA_APP`** | Media streaming budget | HDCP / DRM Video Layer | Video streaming apps (YouTube, Netflix) | Dedicated media video path |

> [!NOTE]
> **Games and GUIs requiring screen output must use `--category 0`.**
> If a GUI or emulator runs under `65536` (`SYSTEM_APP`), `sceVideoOutOpen` fails with `0x80290001 (SCE_VIDEO_OUT_ERROR_INVALID_VALUE)` and `sceKernelAllocateDirectMemory` fails because the kernel grants 0 bytes of DMEM to system apps.
> Running as Category 0 also means `/app0/sce_module/libc.prx` must be present and `pltauth-patch` must be active under kstuff.

Prosperous can push the finished directory to a scan root, where an auto-mounter registers it
without the privileged call at all.

## When you do not know what an entry means

```bash
selfish derive package-a.pkg package-b.pkg package-c.pkg
```

Re-derives what a package's entries mean from packages you supply. This is the tool for
turning "there is an entry here nobody has named" into a row with a provenance, rather than a
guess written into a table.

## Checking what came out

Every format written here is also read here, which is what makes a round trip a test.
[reading.md](reading.md) is that half: `container` and `title` describe what you just built,
and `audit` checks a real file against the format table and reports which rows it settles.
