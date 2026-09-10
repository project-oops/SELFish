# D102 - The title metadata a title carries gets pipeline spellings; `--root` does not

**Status: decided. 2026-09-10.**


**`--content-id`, `--title-version`, `--icon` and `--deeplink` become pipeline options, valid for
`title` and `pkg` and refused elsewhere. `title_dir`'s fifth orphaned parameter, `root`, stays
internal - the pipeline does not expose a caller-supplied directory.**

## What was orphaned, and by what

`title_dir` takes ten things; the `native` and `build title` subcommands that fed five of them -
`content_id`, `version`, `deeplink`, `icon`, `root` - were deleted when the CLI became four axes
(a86f9d9, worklog 062), and nothing replaced them. `title_at` has passed `None` for four and a
staged path for `root` ever since, so a title built through the pipeline could carry a name and
nothing else.

obSCEne surfaced it as a consumer: resolving its build-script fixes (its REQ-4e7a) it recorded that
`--icon` and `--deeplink` "await pipeline flag support in SELFish", and in the meantime its native
title's tile falls back to selfish's default mark. A waiting consumer is the signal that these are
real gaps rather than theoretical ones.

## Four get a spelling

`--content-id`, `--title-version`, `--icon`, `--deeplink`. Each reaches a `param.json` /
`PARAM.SFO` that only `title` and `pkg` write, so each is refused for `elf`, `prx` and `eboot` -
the rule `--category` already follows, for the reason stated there: an option that silently does
nothing is how somebody believes they set one. They travel together through `pkg` → `title` →
`title_dir` in a `TitleMeta` struct, the same shape `WrapOptions` uses for the container options,
for the same reason - a value that survives two hops is clearer gathered than threaded loose.

Two were more than convenience:

- **`--content-id` fixed a latent bug.** The pipeline's `pkg` path passed an **empty** content id
  to `pack`, which keys the filesystem image by a hash of it - so a package built through
  `--format pkg` was keyed to the empty string, an image a console cannot open. The flag threads
  one content id to both the title's `param.json` and the image key, so the two cannot disagree. It
  defaults, announced like `--title-id`, to `UP0000-<title-id>_00-0000000000000000` - a silent
  default here would be exactly the quiet failure D073 is about.
- **`--title-version` is spelled with a prefix** because `--version` is clap's own build-version
  flag on this binary, and the tool reporting its build is worth more than the shorter name. The
  field is renamed too, not just the flag: clap derives an argument's id from the field name, so a
  field called `version` collides with the built-in regardless of its `long`.

## `root` does not

The fifth parameter let a caller point at a directory whose files were copied into the title. The
pipeline deliberately does not take one: `--input` is the executable, and a format writes exactly
`--output` with nothing a caller prepares or cleans. `title_at` still uses `root` internally to
hand `title_dir` the staged eboot - that is the mechanism, not a surface.

The one thing a prebuilt directory used to carry that mattered was a higher-privilege eboot, and
that reason is gone: `--privilege` and `--sdk` reach the pipeline now (D100), so a root-tier title
is built directly rather than by smuggling an eboot in through `--root`. A consumer that wants
extra files in the title - data, `sce_module` - copies them alongside the output, which is what
obSCEne already does. If a real need for a merge-in directory appears, it can be added then; adding
it now would be re-growing the surface the four axes removed, on spec.

## Evidence

Refusals fire for `elf`/`prx`/`eboot` with a message naming the formats that take each option, and
nothing is written. A title built with all four carries them: `param.json` gains `contentId`,
`masterVersion`, `deeplinkUri`, and an `icon0.png`. `--version` still reports the build line. For
`pkg`, an explicit and a defaulted content id both propagate; the empty-id path is gone.

### Correction (2026-09-10): `--icon` was landed incomplete, and the first evidence misread

A review of this commit (SELFish inbox REQ-20260910T1441Z-f979) found `--icon` doing less than this
entry claimed, and the claim itself resting on a misread:

- **The icon was copied, not normalised.** `title_dir` wrote a supplied icon with `std::fs::copy`,
  so an RGBA or non-512 PNG reached `sce_sys/icon0.png` as authored - reintroducing the D073 failure
  (alpha composited wrongly, square on a television) that the `pack` entry path already guards
  against by calling `icon::normalise`.
- **The evidence above was circumstantial.** "A supplied 11,171-byte icon against the 27,069-byte
  default" shows only that the supplied file differs from the default file - not that anything
  converted it. 11,171 bytes was the raw obSCEne logo, copied through. Reading "differs from the
  default" as "was converted" is the same judging-by-association this repository keeps catching;
  the number proved the opposite of what it was cited for.
- **`--icon --format pkg` set only the mounted tile.** The `0x1200` store/pre-install tile stayed
  selfish's default, so a package showed the caller's art after install and the default mark in the
  store.

Fixed in a follow-up: `title_dir` routes `--icon` through `icon::normalise` (which flattens alpha
and **refuses** a non-512 icon rather than resizing - resizing is a decision about the artwork), and
`pkg_from_elf` fills `0x1200` from `--icon`, refusing `--icon` alongside an explicit `--entry
0x1200=` as two sources for one tile. Pinned by `icon.rs` tests that a non-512 icon is refused and
an RGBA one is flattened to RGB. The verification this time reads the re-encoded output and the two
code paths' own messages ("converted to 512x512 RGB" versus "none given, using selfish own"), not a
file-size difference.
