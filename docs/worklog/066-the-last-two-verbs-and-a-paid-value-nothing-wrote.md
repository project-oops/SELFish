# 2026-09-10 - The last two verbs, and a `paid` value nothing wrote


Worklog 062 moved `stamp` and `wrap` onto `--target` but could not delete them, and said exactly
why: `--library` was the only way to a `.prx`, and `--privilege`/`--sdk` had no pipeline spelling.
Three axes the verbs carried and the pipeline did not. This is closing all three, which empties
the verbs, which deletes them.

## The change, which is small

- `--format prx` joins `elf`. It is the same stamp with `ObjectType::SharedLibrary`, which is the
  whole of what `stamp --library` did. A shared library is a *kind of output*, so it belongs on
  the axis that selects output kinds rather than as a flag on another one.
- `--privilege` and `--sdk` become pipeline options, gathered into a `WrapOptions` struct and
  threaded through `eboot` → `title` → `pkg`, since each builds the next. Valid for the three
  formats that build a container; refused for `elf` and `prx`, which build none, with a message
  naming the formats that do apply. That is the rule `--category` already followed.
- `stamp` and `wrap` deleted: the variants, the dispatch arms, the two functions.

Then the documentation that named them: README's command list, docs/README's "two layer verbs"
paragraph, the `self-format.tsv` note about `wrap`'s default, and writing.md - which is rebuilt
from real output, captured from obSCEne's probe eboot and its `libc` module rather than a payload
this repository does not have.

The default-target reasoning in D097 now has no subject - `wrap` was the only thing that defaulted
- so D097 gets a banner rather than an edit, because a decision is a record of why something was
done and not a thing to rewrite when it ends.

## What I did not trust, and was right not to

Two claims about the `paid` value were sitting in the docs, and both were wrong the same way.

The `Privilege` enum's own doc comment, and the table in writing.md, said `app` stamps
`0x3800000000000000`. I was about to repeat it in the new CLI help. Instead I built a container at
each tier and read the value back with `audit`:

```
app         paid 0x3100000000000002
sysmodule   paid 0x3100000000000002
system      paid 0x3800000000000001
root        paid 0x8000000000000001
```

`app` is `0x3100000000000002`, the format default. `0x3800000000000000` is written by nothing.
And `sysmodule` is **byte-for-byte identical to `app`** - `Privilege::paid` maps both to the
default - which `cmp` confirmed on two full containers. The docs implied four distinct tiers; there
are three distinct byte patterns and a fourth name that aliases the first.

This is the D099 shape again, one decision later: a value believed because a comment asserted it,
refuted by the artifact the comment describes. The fix each time is the same - read the thing, not
its label - and it is becoming the most reliable single move in this repository.

## A consumer has been losing privileges silently

While checking what `--privilege` should do, I audited obSCEne's native title. Its Makefile passes
`PRIVILEGE=root` to `build-native.sh`, and the script builds a `root_arg=(--root …)` pointing at a
staged root-tier eboot - then calls `selfish --input … --format title` **without expanding it**,
along with dropped `--icon`, `--deeplink` and `--category` arrays. The installed title,
`build/prospero/PPSA99980/eboot.bin`, carries `paid 0x3100000000000002` (App); the staged
`native-root/eboot.bin` it meant to use carries `0x8000000000000001` (Root).

That is a shell-layer bug in obSCEne - `selfish` cannot honour a flag a caller never passes - and
its deeper cause is mine: when `native` and `build title` were deleted (a86f9d9), the parameters
those arrays fed (`--root`, `--icon`, `--deeplink`, `--category`, content id, version) lost their
CLI spelling, but `title_dir` still takes every one of them and `title_at` passes `None` for all.
The capability is dead code on our side and silently dropped arguments on theirs.

Filed to obSCEne (REQ, see its inbox), and spun off two SELFish follow-ups: one to decide each
unreachable `title_dir` parameter's fate - spelling or deletion - and one to stop `--format title`
printing a scratch path that no longer exists by the time the command returns (which writing.md has
to elide by hand, and which is an absolute machine path the guard would otherwise block).

## The gap I am leaving on purpose

The container builder writes `e_type EXECUTABLE` no matter the payload. So `--format prx` makes a
correctly stamped shared-library ELF, but nothing wraps a `.prx` *in a container* as a library -
obSCEne's modules go through its own `mkself`. Closing it means the container builder taking an
object type, which is a bigger change than emptying two verbs. D100 records it so it is visible
rather than found.
