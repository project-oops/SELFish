# User guide

`selfish` has two ways in. A build is one invocation of four options; everything else is a
subcommand that reads a file, or builds one part of a package. `selfish --help` and
`selfish <subcommand> --help` are the authority for every option.

## Building

```text
selfish --input <elf> --target <orbis|neo|prospero|trinity> --format <elf|prx|eboot|title|pkg> --output <path>
```

- `--target` names the machine and so the generation: `orbis` and `neo` are Orbis-generation,
  `prospero` and `trinity` Prospero-generation. `neo` and `trinity` are the mid-generation
  refreshes and are only for builds specific to them. There is no default (D002).
- Each format writes exactly `--output` and nothing beside it. A format that needs a staging
  directory makes one under the system temp directory and removes it.
- An option a format does not use is refused, naming the formats that take it (D100).

The output lists each step: the fields stamped, the container size, every file written and
every default taken.

### `--format elf` and `--format prx`

The module, with the platform identity no linker sets: `EI_OSABI`, `EI_ABIVERSION`, `e_type`
(executable for `elf`, shared library for `prx`), and on Prospero-generation targets every
read-and-execute load segment made execute-only. Only fields that change are reported. A module
stamped this way is what a homebrew loader maps directly.

```console
$ selfish --input payload.elf --target prospero --format elf --output module.elf
Prospero -> Elf  (prospero)
  elf     6 program header(s), entry 0x531270
  stamp   EI_ABIVERSION  0x0 -> 0x2
  stamp   e_type         0xfe00 -> 0xfe10
  stamp   p_flags (R+X -> X) 0x5 -> 0x1
  wrote   module.elf (5598248 bytes)
```

### `--format eboot`

The stamped executable inside a signed-executable container. The container declares itself fake
and its signature area is zero (D047).

- `--privilege app|sysmodule|system|root` is the tier the container declares; the default is
  `app`, and `sysmodule` writes the same container as `app`.
- `--sdk VERSION` pins the SDK version in `PT_SCE_PROCPARAM` and the container, as a version
  (`2.000.009`) or an alias from `data/sdk-versions.toml` (`prospero`, `orbis`, ...). A version
  for the other generation is refused. `--sdk-table FILE` reads another version table.

```console
$ selfish --input payload.elf --target prospero --format eboot --privilege system --sdk prospero --output eboot.bin
Prospero -> Eboot  (prospero)
  stamp   0 field(s)
  tier    System
  sdk     0x08050001/0x02000009
  wrap    5146000 bytes from a 5301368 byte payload
  wrote   eboot.bin (5146000 bytes)
```

### `--format title`

A title directory at `<output>/<TITLE_ID>/`: the `eboot.bin`, and `sce_sys/` with
`param.json`, `icon0.png`, `pic0.png`, `logo.png`, `keystone`, `pfs-version.dat` and
`nptitle.dat`. That is the shape `sceAppInstUtilAppInstallTitleDir` installs; nothing here
installs it (D082).

| Option | Sets | Default |
|---|---|---|
| `--title-id` | the id: four capital letters then five digits | `OBSC00001`, announced |
| `--title` | the name on the home screen | the title id |
| `--category` | `big-app`, `system-app`, `mini-app`, `daemon` or `media-app` | `system-app` |
| `--content-id` | `contentId` in `param.json` | `UP0000-<title id>_00-0000000000000000`, announced |
| `--title-version` | the version and master version, `NN.NN` | none written |
| `--subtitle` | `titleSubName` | `OOPS Native Title` |
| `--deeplink` | a URI the tile launches instead of its own executable | none |
| `--icon`, `--pic0`, `--logo` | the tile (512x512), background (1920x1080 or 3840x2160) and logo PNGs | selfish's own artwork |

A malformed title id installs and is then never indexed, so it is refused. `--category` decides
the memory budget and display ownership: a title that draws must not be `system-app`, which gets
no direct memory. The category values are in the collection's
[four-axes table](https://github.com/project-oops/OOPS/blob/main/docs/CONVENTIONS.md#the-four-axes-of-a-build-and-a-run).
`--privilege system` also sets the category to `0x20000`.

A supplied PNG is flattened over black to RGB and refused, not resized, if its size is wrong
(D073).

### `--format pkg`

An installable package built around the same title layout. It installs through the
compatibility path. `--content-id` matters here: the filesystem image is encrypted under a key
derived from it, so a package whose declared id differs cannot be opened. `--icon` also fills
the package's `0x1200` store tile. `--entry ID=FILE` supplies an entry the builder would
otherwise compute or generate; none is required (D099).

## Building a package in parts

`image` and `pack` are the two halves of `--format pkg`, for iterating on one while the other
stays fixed.

```console
$ selfish image --root title/TEST00001 --out img.bin --content-id UP0000-TEST00001_00-0000000000000000
sce_sys/keystone: generated from the passcode
img.bin: 6881280 bytes
keyed to UP0000-TEST00001_00-0000000000000000 - a package carrying this must declare the same id

$ selfish pack --image img.bin --out title.pkg --content-id UP0000-TEST00001_00-0000000000000000
param.sfo: generated for TEST00001 ("TEST00001", version 01.00)
icon0.png: none given, using selfish own - supply --entry 0x1200=FILE to replace
playgo-manifest.xml: generated the default manifest
title.pkg: 7405568 bytes, 14 entries, image at 0x80000
1 gap(s) left blank, because nothing established says what goes in them:
  entry 0x80 at 0x60, 32 bytes - the header digest, filled by finalize_digests once the header exists
```

- `image` builds a plain filesystem from a directory, wraps it in `PFSC`, and carries that as
  the single file of a signed and encrypted outer filesystem. It adds `sce_sys/keystone` from
  the passcode unless the directory has one.
- `pack --dir DIR` runs `image` first; `pack --image FILE` takes a built one. `--passcode`
  defaults to the fake one for both.
- `pack` computes the digest tables, block digests, licences, key blobs and entry tables,
  generates `param.sfo` from `--title-id`, `--title` and `--version`, and reports every region it
  left blank (D037).
- `pack` warns when the inner filesystem is too small for the hardware to mount; the fix is a
  larger title, not a smaller declared cache (D071).

## Shader containers

```bash
selfish shader --out shader.bin --shader-size 256 --sh-reg 0x20c=0 --sh-reg 0x20d=0
```

Builds the AGC container `sceAgcCreateShader` takes. `--stage` is `compute` (the default),
`pixel`, `vertex`, or a raw type value. `--code FILE` records the bytecode's size in place of
`--shader-size`; the bytecode is not embedded. The register contents are the caller's (D103).

## Reading a file

Every reading command takes a path and changes nothing. An executable inside a container is
unwrapped first.

| Command | Says |
|---|---|
| `selfish container FILE` | the container's generation, entries, sizes, and where the executable sits |
| `selfish elf FILE` | type, generation, OSABI, entry, segments, and the dynamic table's convention, tables and libraries |
| `selfish imports FILE [--all]` | imports resolved to library and module, counted per library or listed in full |
| `selfish sections FILE [--defines NAME]...` | section headers and the link-time symbol table, and whether each name is defined |
| `selfish reloc FILE` | relocations counted by type, and how the PLT slots join to the imports |
| `selfish nid NAME...` | each name's import hash |
| `selfish title FILE [--round-trip]` | what a package, `PARAM.SFO` or `param.json` says about its title; `--round-trip` writes it back and compares |
| `selfish pkg FILE [--all]` | a package's entries and its first forty files, or all of them |
| `selfish extract FILE OUT` | every file of a package, refusing paths that would leave `OUT` |
| `selfish derive PKG...` | re-runs the derivations behind the `DERIVED` rows of `data/pkg-format.tsv` (D035) |
| `selfish audit FILE` | a real container's header and `ex_info` rows against `data/self-format.tsv` (D084) |

`audit` prints what kind of container it read before how many rows agree, because a fake
container written from the table agrees with every row. A difference is reported with both
values and not interpreted.

Output that is piped into a reader that stops early, such as `head`, ends quietly with status
zero.

## Using the crates

Each library crate depends only on those before it in `CLAUDE.md`'s spine, so a consumer takes
the one it needs: a loader reading executables takes `selfish-elf` and compiles no cipher.
Builders take the generation as a `Generation` with no default. The format tables in `data/`
are compiled in, and the crates carry no copy of their numbers.

`crates/*/examples/` holds probes that print what a real file contains, for checking a
structure against material: `cargo run -p selfish-pkg --example wrap_keys -- <package>`.
