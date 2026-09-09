# 2026-09-09 - Thirty-two containers, a clean split, and a confound I had already glossed


obSCEne rewrote `048-selfaudit` to walk every installed application tree and report each
container's `ex_info.ptype` **before** its verdict - the change asked for so that a `confirmed`
could never again be read without the one fact that says whether it means anything. Thirty-two
containers audited.

The split is total:

```
ptype 0x1 (PTYPE_FAKE)   9 containers    all NINE fixed rows match the table
ptype 0x0               23 containers    the same FIVE rows differ, identically, on every one
```

Nine for nine and twenty-three for twenty-three, no partial cases either way. **No container
that is not marked fake matches this table, and every container that is, does.**

Which confirms the reversal two entries ago was right, and explains it better than it explained
itself: the table came from OpenOrbis, a fake-SELF **writer**, so it describes what that writer
produces. Agreement is a property of the lineage, not a measurement.

## The confound, which the last entry glossed

The reversal said the limit was *categorical* - "these five rows describe an application
container". Reading the log rather than the summary shows that was one arm of a confound read as
a cause:

- every `ptype 0x1` container is **PPSA**-shaped, a game title id;
- every `ptype 0x0` container is **NPXS**-shaped, a system application.

There is not one genuine application container on the box and not one fake system one. So the
five rows may describe *a fake container* or *an application container*, and this sample cannot
tell them apart. The fake reading is better supported - it is what the source wrote, and `ptype`
is literally the field the format provides for saying so - but it is not established.

What would separate them is a genuine retail game, or a fake system-tier container. The console
has neither, so the question is not answerable from it, and the request that would have asked
again is not being filed. That was written into 9c40's own acceptance: a statement that the
console cannot settle it is terminal.

## Two smaller things from reading the log

**`FAKE00000` again.** obSCEne's summary lists it among the nine fakes; the log has it at
`ptype 0x0`, diverging, grouped with the genuine system containers. The log is right. That is
the third time today that reading the data rather than the prose changed the answer, and the
second time on this particular file - which was never ours, and is not fake either, whatever its
name says.

**The tail sample tripled.** Worklog 051 argued that seven files of different sizes agreeing on
three of four `ex_info` fields is not what a misaligned read produces. It is now twenty-three,
and the agreement held. Further past coincidence than it was.

## What this closes

The whole chain from D084 - build the audit, get a real container, compare - has now run to the
end, and the answer is that this console cannot confirm the table against vendor material,
because it holds no genuine container of the shape the table describes. That is a smaller result
than the one claimed for an hour this afternoon and it is the one that is true.

The limit in `data/self-format.tsv` stands, better worded: not "this is the previous console's
container", but "every row here describes what a fake-SELF writer produces, and no genuine
container measured agrees with five of them".
