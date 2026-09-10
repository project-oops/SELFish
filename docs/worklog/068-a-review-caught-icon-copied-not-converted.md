# 2026-09-10 - A review caught `--icon` copied, not converted - and my evidence that it was


D102 added `--icon` to the pipeline the same day, and a review session diffing the commit
(SELFish inbox f979) found it doing less than its help said, resting on a verification I had
misread. Both halves are worth recording, because the second is the more embarrassing and the
more instructive.

## What the flag actually did

`title_dir` wrote a supplied icon with `std::fs::copy(path, &icon_path)` - a raw copy. The flag's
help said "converted to the 512x512 RGB a console wants (D073)", and the `pack` entry path one
screen away *did* convert, through `icon::normalise`. The title path just did not call it. So an
RGBA or off-size PNG went to `sce_sys/icon0.png` as authored, reintroducing exactly the D073 fault
- an icon with alpha composited wrongly, reading square on a home screen, found on a television -
for anyone exporting art that was not already right.

And `--icon --format pkg` set only the mounted tile. `pkg_from_elf` passed `entries` to `pack`
untouched, so the `0x1200` store tile stayed selfish's default: caller art after install, default
mark in the store.

## What I told myself when I landed it

D102's evidence read: *"an icon0.png that differs from the generated default - a supplied
11,171-byte icon against the 27,069-byte default."* I wrote that as proof the icon was converted.
It proves nothing of the kind. 11,171 bytes was the raw obSCEne logo; "differs from the default
file" only says the supplied file is not the default file, which a copy satisfies as well as a
conversion. I had the exact byte count of the unconverted input sitting in the evidence line and
read it as evidence of conversion.

This is the repository's recurring fault in its purest form yet: **judging what happened from a
number that was consistent with it, rather than from the thing itself.** The discriminator I
should have used - and did, this time - is to read the re-encoded output (11,171 -> 40,876 bytes,
and the code's own "converted to 512x512 RGB" line versus "none given, using selfish own") rather
than a size that differs for two different reasons.

## The fix

- `title_dir` routes `--icon` through `icon::normalise`, like the `pack` path. `normalise` flattens
  alpha over black and **refuses** a non-512 icon rather than resizing - scaling is a decision
  about the artwork, so it names the size and declines. The mounted icon is now re-encoded
  (measured: 40,876 bytes, RGB), not copied.
- `pkg_from_elf` fills the `0x1200` store tile from `--icon`, injected as a `0x1200` entry so it
  goes through `pack`'s own `normalise`. `--icon` alongside an explicit `--entry 0x1200=` is
  refused - two sources for one tile, the pattern the pkg builder already uses for its computed
  entries.
- The `--icon` help and writing.md now say what `normalise` does: flatten alpha, refuse a non-512
  icon, and for `pkg` fill both tiles. D102 carries a correction recording the copy, the misread,
  and the missing store tile.
- Two tests in `icon.rs`, which is where this kind of guarantee is pinned: a non-512 icon is
  refused with its size named, and a 512 RGBA icon comes out RGB. Both call sites route through the
  function these pin.

## The thing to carry

The review was another session reading my own commit, and it found both the defect and the bad
evidence. That is the second reader the CLAUDE.md argues for, working exactly as described - and a
reminder that "I verified it" is only worth as much as what the check could have distinguished. A
size that changes whether or not the thing I claim happened, is not a check. (D102 correction, f979)
