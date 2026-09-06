# Producing something the hardware will load

For most workflows, **`selfish build`** runs the complete assembly pipeline in a single step:
- `selfish build pkg`: Orchestrates ELF stamping, SELF wrapping, `param.sfo`, PFS image, and `.pkg` assembly.
- `selfish build title`: Orchestrates Prospero SELF wrapping, `param.json`, `icon0.png`, keystone, and `<TITLE_ID>/` title staging.
- `selfish build payload`: Validates and stamps ELF shared objects for `elfldr`.

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
$ selfish stamp payload.elf --generation 5
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
$ selfish wrap payload.elf --out eboot-gen4.bin --generation 4
eboot-gen4.bin: 30368 bytes from a 35720 byte payload, previous generation (privilege: App, sdk: 0x08008011/0x00000000)

$ selfish wrap payload.elf --out eboot-gen5.bin --generation 5 --sdk ps5-native
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

**`--generation` defaults to 4, and that is a measurement rather than a habit.** Every
container found inside the real current-console packages sampled carries the *previous*
generation's magic - D010 is the count, a working homebrew store among them. Pass
`--generation 5` when you mean it; the default is what the evidence says is normal.

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
$ selfish build title --out native --title-id OBSC00001 --title "Demo" --content-id UP0000-OBSC00001_00-0000000000000000 --root app
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

(The command `selfish native` is retained as a deprecated alias pointing to `selfish build title`.)

Four of the five metadata files are generated because they are derivable; the eboot came in
through `--root`, which copies extra files verbatim (and wraps `eboot.elf` into `eboot.bin` automatically
if it has not already been wrapped). This lays out bytes and then says what it cannot do - the registration
call is not one a host-side program can make, so it names the call rather than appearing to install anything.

`--deeplink` makes an entry that launches something else instead of carrying its own
executable, which is the shape to use when the code is already running as a payload.
`--category` and `--privilege` default to what a Prospero homebrew entry uses.

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
