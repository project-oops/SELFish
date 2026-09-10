# 2026-09-10 - The pipeline learns what a title carries, and an empty content id that keyed nothing


Retiring `stamp` and `wrap` (D100) left the pipeline as the only way to build a title - and a title
it built could carry a name and nothing else. `title_dir` takes a content id, version, deeplink,
icon and an extra-files root; the subcommands that fed them went with the verbs in a86f9d9, and
`title_at` had passed `None` for four of them ever since. This wires four back, and declines the
fifth.

## The consumer that made it not-theoretical

obSCEne resolved its own build-script fixes and noted, in passing, that `--icon` and `--deeplink`
"await pipeline flag support in SELFish" - so its native title's tile was falling back to selfish's
default mark because there was no way to pass the logo. A gap with someone standing at it is worth
more than a tidy one.

## Four spellings, and the one that was a bug

`--content-id`, `--title-version`, `--icon`, `--deeplink`, gathered into a `TitleMeta` struct the
way the container options are gathered into `WrapOptions`, threaded `pkg` -> `title` ->
`title_dir`, and refused for `elf`/`prx`/`eboot` the way `--category` is. Title metadata reaches a
`param.json`; a format that writes no `param.json` has nowhere to put it, so passing it there is an
error, not a no-op.

Two were more than plumbing:

- **`--content-id` closed a latent bug.** The pipeline's `pkg` path handed `pack` an **empty**
  content id. `pack` keys the filesystem image by a hash of the content id, so every package built
  through `--format pkg` was keyed to the empty string - an image a console cannot open. Nobody had
  hit it because obSCEne drives `selfish pack` directly, not `--format pkg`; the pipeline pkg leg
  had simply never been asked to produce something installable. The flag threads one id to both the
  title's `param.json` and the image key, so they cannot disagree, and defaults - announced, like
  the title id - rather than silently keying to a placeholder.
- **`--title-version`, not `--version`.** `--version` is clap's own build-version flag on this
  binary, and `selfish --version` reporting its build is worth keeping. The first rename only
  changed the `long`, and it still panicked at startup: clap derives an argument's id from the
  *field name*, so a field called `version` collides with the built-in whatever you call its flag.
  Renaming the field fixed it. A good reminder that the visible name and the identity are two
  different things - the same shape as the orphaned attribute in worklog 062.

## The fifth stays internal

`root` let a caller merge a directory into the title. The pipeline does not take one: `--input` is
the executable and a format writes exactly `--output`. Its one load-bearing past use - smuggling in
a higher-privilege eboot - is moot now that `--privilege`/`--sdk` reach the pipeline (D100), so a
root-tier title is built directly. A consumer wanting extra files copies them beside the output,
which obSCEne already does. Re-adding a caller-prepared directory on spec would be regrowing exactly
the surface the four axes removed. (D102)

## Checked

Refusals fire for the three formats that carry no metadata, naming the ones that do, and write
nothing. A title built with all four carries them - `contentId`, `masterVersion`, `deeplinkUri`,
and an icon that differs in size from the generated default, so it is the supplied one. `pkg`
propagates an explicit content id and a defaulted one alike; the empty-id path is gone. `--version`
still prints the build line. Gate green; identity scan clean.
