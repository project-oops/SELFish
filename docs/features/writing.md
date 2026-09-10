# Producing something the hardware will load

For most workflows, **the pipeline** runs the complete assembly in a single invocation:

```
selfish --input <file> --target <orbis|neo|prospero|trinity> --format <elf|prx|eboot|title|pkg> --output <path>
```

- `--format elf`: stamps the platform identity a loader checks before anything else.
- `--format prx`: the same stamping, for a shared library.
- `--format eboot`: the executable, stamped and wrapped in a signed-executable container.
- `--format title`: container plus `param.json`, `icon0.png`, keystone, laid out as `<output>/<TITLE_ID>/`.
- `--format pkg`: that, plus `param.sfo` and a PFS image, assembled into a `.pkg`.

`--target` carries the generation and is always required. Nothing passes a generation number, and
nothing picks one for you.

Underneath, between a compiler and the hardware there are four steps, and then a fork. The first
two are pipeline formats - `elf` and `prx` stop after stamping, `eboot` after wrapping - and the
last two are commands of their own, `image` and `pack`, so a package can be built a layer at a
time. That matters, because when a file is rejected the useful question is *which step produced
the wrong bytes*.

```
   compiler output
        |  stamp     the platform identity no linker sets      --format elf, prx
        v
      module  ------------------------------------> a payload, and it can stop here
        |  wrap      the signed-executable container           --format eboot
        v
    eboot.bin
        |                              \
        |  image     the filesystem     \  --format title  lay out a Prospero title
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

Every transcript below is real output, from obSCEne's probe executable (5.6 MB) and its `libc`
module (166 KB), captured on Windows - hence the separators in step 4b, where one temporary path
is also elided.

## 1. Stamp the identity

```console
$ selfish --input payload.elf --target prospero --format elf --output module.elf
Prospero -> Elf  (current generation)
  elf     6 program header(s), entry 0x531270
  stamp   EI_ABIVERSION  0x0 -> 0x2
  stamp   e_type         0xfe00 -> 0xfe10
  stamp   p_flags (R+X -> X) 0x5 -> 0x1
  wrote   module.elf (5598248 bytes)
```

Writes exactly `--output`, and says which fields it changed and what they were. No linker sets
these, because no linker knows about either console - so a freshly compiled ELF is not yet a
module, and nothing about it looks wrong until a loader refuses it.

A shared library is the same step with a different `e_type`, and that is `--format prx`:

```console
$ selfish --input libc.elf --target prospero --format prx --output libc.prx
Prospero -> Prx  (current generation)
  elf     5 program header(s), entry 0x0
  stamp   EI_ABIVERSION  0x0 -> 0x2
  wrote   libc.prx (166320 bytes)
```

One field this time. The input already carried `e_type` `0xfe18`, the shared-library value, so
only the ABI version was left to write - stamping reports what it changed, not what it checked.

**A stamped file is already a payload.** If a homebrew loader is going to map and run it, the
remaining steps are not needed at all.

## 2. Wrap it in a container

```console
$ selfish --input payload.elf --target orbis --format eboot --output eboot-gen4.bin
Orbis -> Eboot  (previous generation)
  stamp   1 field(s)
  wrap    5469936 bytes from a 5598248 byte payload
  wrote   eboot-gen4.bin (5469936 bytes)

$ selfish --input payload.elf --target prospero --format eboot --sdk ps5-native --output eboot-gen5.bin
Prospero -> Eboot  (current generation)
  stamp   3 field(s)
  tier    App
  sdk     0x08050001/0x02000009
  wrap    5469936 bytes from a 5598248 byte payload
  wrote   eboot-gen5.bin (5469936 bytes)
```

`--format eboot` stamps on the way, so it takes the compiler's output directly rather than a
module stamped beforehand.

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

**There is no default `--target`.** Both generations are accepted by current hardware, each
proven on its own delivery route: a *package* with the previous generation's magic installs,
mounts, loads and executes (worklog 040), and a *native title* with the current magic installs,
launches and ran 292 checks to completion (obscene sweep 20260909-184538). Neither has been shown
accepted on the other's route, so a default would be right for one route and wrong for the other.

The retired `wrap` verb did default, to `orbis`, because that served the package path. Its
justification once included a population - "every container inside the real packages sampled
carries the previous generation's magic" - and **that was withdrawn**: those containers were of
homebrew lineage, and every *genuine* container measured carries the current magic, 23 of 23.
(D097, D100)

**`--sdk` takes an alias or a literal version.** `ps5-native` resolved to
`0x08050001/0x02000009` above; `2.000.009` and `ps4-compat` are equally valid. The dictionary
is `data/sdk-versions.toml`, outside the source, because a version table is data rather than
code, and it is validated against the generation - a current-generation version on a
previous-generation container is refused rather than written. Getting this wrong is not
subtle: the loader refuses the process with an SDK-version error before any of your code
runs. `--sdk-table` reads a different dictionary.

**`--privilege` takes `app`, `sysmodule`, `system` or `root`**, and defaults to `app`. Asking for
`app` produces the same bytes as not asking - checked with `cmp`, not assumed:

```console
$ selfish --input payload.elf --target prospero --format eboot --privilege root --sdk ps5-native --output eboot-root.bin
Prospero -> Eboot  (current generation)
  stamp   3 field(s)
  tier    Root
  sdk     0x08050001/0x02000009
  wrap    5469936 bytes from a 5598248 byte payload
  wrote   eboot-root.bin (5469936 bytes)
```

Both options apply to the formats that build a container - `eboot`, `title` and `pkg` - and are
refused with `elf` and `prx`, which build none. The refusal names the formats they do apply to,
rather than accepting a tier there is no container to put in.

The container declares itself fake in the field the format provides for exactly that, and its
signature area is zero. No vendor signature is forged and none could be.

## 3. Build the filesystem image

`app` here holds the previous-generation `eboot.bin` from step 2.

```console
$ selfish image --root app --out title.pfs.img --content-id UP0000-OBSC00001_00-0000000000000000
sce_sys/keystone: generated from the passcode
title.pfs.img: 6553600 bytes
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
$ selfish pack --image title.pfs.img --out title.pkg --content-id UP0000-OBSC00001_00-0000000000000000 --title-id OBSC00001 --title Demo
param.sfo: generated for OBSC00001 ("Demo", version 01.00)
icon0.png: none given, using selfish own - supply --entry 0x1200=FILE to replace
playgo-manifest.xml: generated the default manifest
title.pkg: 7077888 bytes, 14 entries, image at 0x80000
1 gap(s) left blank, because nothing established says what goes in them:
  entry 0x80 at 0x60, 32 bytes - the header digest, filled by finalize_digests once the header exists
```

**Nothing was supplied, and nothing was guessed.** Every entry is computed or generated: both
digest tables, the block digests, both licences, both key blobs, `param.sfo`, the playgo
manifest, the entry name table and the playgo chunk table.

The last two used to be demanded, and this section used to show `pack` stopping without them.
Both turned out to be facts the builder was already holding: the name table is a function of
which entries are present, and the chunk table is fixed for a single-chunk title apart from two
sizes it can read out of the image. (D099)

What `pack` still does is list every region it left blank rather than hiding it, and say what it
knows about each.

`--entry ID=FILE` overrides any entry, which is for rebuilding a package to match existing
material - and for the one entry that is genuinely yours, the icon:

```bash
selfish pack --image title.pfs.img --out title.pkg \
    --content-id UP0000-OBSC00001_00-0000000000000000 \
    --entry 0x1200=icon0.png
```

Given an image whose inner filesystem is too small to mount, `pack` warns before anything else.
The threshold, the error string and the workaround that *did not* work were all established
against hardware. (D071)

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
repository. obscene#D255 is the finding.

## 4b. Or lay out a title directory instead

The current generation's (Prospero) own format is not a package at all, and this is the other half of
the fork. It is a directory described by `param.json`, registered by a call that needs kernel
privileges:

```console
$ selfish --input payload.elf --target prospero --format title --title-id OBSC00001 --title Demo --output native
Prospero -> Title  (current generation)
  stamp   3 field(s)
  wrap    5469936 bytes from a 5598248 byte payload
  wrote   <scratch>\root\eboot.bin (5469936 bytes)
1 file(s) copied from <scratch>\root
privilege: App
native\OBSC00001\sce_sys\param.json
native\OBSC00001\sce_sys\icon0.png (generated)
native\OBSC00001\sce_sys\keystone (generated)
native\OBSC00001\sce_sys\pfs-version.dat (generated)
native\OBSC00001\sce_sys\nptitle.dat (generated)
  title   native\OBSC00001
```

`<scratch>` is a temporary directory the eboot is staged in and removed before the command
returns.

The eboot is built from `--input` on the way - stamped, wrapped, staged - and the title is laid
out around it. Four of the five metadata files are generated because they are derivable. Nothing
is installed: registration is `sceAppInstUtilAppInstallTitleDir("OBSC00001", "/user/app/", 0)`
after copying the directory to `/user/app/`, and that call needs kernel privileges, so it runs
from a payload on the target rather than from here.

`--category` defaults to `system-app` and `--privilege` to `app`.

### Choosing `--category` and `--privilege`

Execution on PlayStation systems is governed by two orthogonal axes:

1. **Privilege Tier (`paid` / Authority ID in the SELF header)**. The values are what `selfish`
   writes, read back with `audit`; what each tier is granted on hardware is not measured here.
   - `app` (`0x3100000000000002`, the format's default): Standard sandboxed execution.
   - `sysmodule` (`0x3100000000000002`): **currently identical to `app`**, byte for byte -
     nothing distinguishes the two when the container is built, so choosing it changes nothing.
   - `system` (`0x3800000000000001`): Unsandboxed system service privileges.
   - `root` (`0x8000000000000001`): Full kernel root authority (jailbreak syscalls, arbitrary `/dev/*` nodes).

   `--privilege system` also sets the title's category to `0x20000`, a pairing carried over
   rather than measured.

   An earlier copy of this list gave `app` as `0x3800000000000000` and `sysmodule` as
   `0x3800000000000001`. Nothing writes either.

2. **Application Category (`applicationCategoryType` in `param.json`, `CATEGORY` in
   `PARAM.SFO`, `--category` on the pipeline)**: decides Direct Memory allocation, HDMI
   scanout ownership and process lifecycle.

   **The values live in one place**: [the collection's four-axes
   table](https://github.com/project-oops/OOPS/blob/main/docs/CONVENTIONS.md#the-four-axes-of-a-build-and-a-run),
   and `selfish_title::category` for code. This table used to be repeated in five places and
   two of the copies had already drifted, which is the argument against a sixth.

   The one consequence worth stating here, because it is what people hit: **a GUI or emulator
   that needs the screen must not be a system app.** Under `system-app` the kernel grants zero
   direct memory and `sceVideoOutOpen` fails `0x80290001`, so the app starts and then cannot
   draw. `big-app` is the category with the display, and it brings its own requirements -
   `/app0/sce_module/libc.prx` present, and `pltauth-patch` active under kstuff.

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
