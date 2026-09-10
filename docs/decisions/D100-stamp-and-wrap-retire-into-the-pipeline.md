# D100 - `stamp` and `wrap` retire into the pipeline; `--format prx`, and `--privilege`/`--sdk` as pipeline options

**Status: decided. 2026-09-10.**


**The `stamp` and `wrap` subcommands are deleted. A shared library is `--format prx`, and
`--privilege`/`--sdk` are pipeline options on the formats that build a container. The pipeline is
now the only way to stamp or wrap.**

## Why they were still here

Worklog 062 moved `stamp` and `wrap` onto `--target` but kept both verbs, naming exactly three
reasons they could not yet go:

- `--library` on `stamp` was the only route to a `.prx`; no `--format` produced one.
- `--privilege` and `--sdk` on `wrap` had no pipeline spelling.

Those were real gaps, not habits, and CONVENTIONS §7 forbids a superseded verb surviving on
anything less. Each is closed by giving the pipeline the axis the verb was carrying:

- **`--format prx`** stamps a shared library. An executable and a `.prx` differ at this layer by
  one field, `e_type`, which is exactly what the `--format` axis selects - so a `.prx` being a
  format value rather than a flag on `elf` is the same distinction the axis already draws. The
  deleted `stamp --library` did precisely this: chose `ObjectType::SharedLibrary` over
  `Executable` and stamped.
- **`--privilege` and `--sdk`** become options, valid for `eboot`, `title` and `pkg` - the formats
  that build a container - and refused for `elf` and `prx`, which build none. The refusal names
  the formats they do apply to. This is the rule `--category` already follows (D-category in
  worklog 062): an option that silently does nothing is how somebody believes they set something.

With all three carried by the pipeline, the verbs have nothing left, and they are gone rather than
deprecated.

## What this does not change

**The bytes.** `--privilege app` with no `--sdk` routes through `selfish_container::build`, the
same call the default path always used; `cmp` confirms `--privilege app` produces a container
identical to omitting the flag. Only a non-`App` tier or an explicit `--sdk` reaches
`build_with_options`, so an ordinary run does not pay for a version-table load it does not read.

**The default target.** `wrap` defaulted to `orbis`; the pipeline has never had a default
`--target` and still does not. D097's reasoning about that default now has no subject, and D097
carries a banner saying so. A container's magic is named by naming its machine, which is required.

## Two things found while doing it, neither invented here

- **`sysmodule` is identical to `app`.** `Privilege::paid` maps both to the format default, so a
  container built at `--privilege sysmodule` is byte-for-byte one built at `app`. That is the
  existing behaviour of `selfish-container`, surfaced by giving the tier a pipeline spelling;
  this decision does not change it, but the help and `writing.md` now say it rather than implying
  four distinct tiers. Whether the two *should* differ is a question for whoever has measured what
  sysmodule grants, which nobody here has.
- **A `paid` table that was written by nothing.** The `Privilege` doc and `writing.md` both listed
  `app` as `0x3800000000000000`. Read back from a built container with `audit`, `app` is
  `0x3100000000000002` - the format default - and `0x3800000000000000` appears nowhere the builder
  writes. Corrected against the artifact rather than against the comment. (Same shape as the field
  names in D099: a value believed from a doc, refuted by the thing itself.)

## What this deliberately leaves

The container builder stamps `e_type` `ObjectType::EXECUTABLE` into the header it writes,
regardless of the payload. So `--format prx` produces a correctly stamped shared-library *ELF*,
but there is no `--format` that wraps a `.prx` *in a container* as a library - obSCEne's `.prx`
modules are wrapped by its own `mkself`. Closing that would mean the container builder taking an
object type, which is a larger change than retiring two verbs and is not part of this. Recorded so
the gap is visible rather than discovered.
