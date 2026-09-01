# D071 - D070's rule was wrong, and the corrected one cannot be tested by avoiding it


D070 concluded that a console refuses an image whose inner filesystem is smaller than the cache
its header declares, and had `selfish pack` clamp the declared value down to fit. The evidence was
one A/B: padding a small package's inner image past `0xD0000` cleared the error.

**That was the wrong reading, and the clamp does nothing.** Built with the declared cache set to
exactly the inner size and sent to a console, the mount fails identically:

| declared cache | inner image | result |
|---|---|---|
| 851968 | 720896 | `Failed to enable GDDR5 cache`, EINVAL |
| **720896** | 720896 | **the same, at the same line** |
| 851968 | 1769472 | mounts past it |
| 851968 | 11141120 | mounts past it |

The declared field does not decide it. **The inner filesystem itself has to exceed a fixed size**,
somewhere in `(720896, 1769472]` - `0xD0000` is the plausible boundary and is *not* established;
it is only the value the header happens to carry. The padding A/B changed two things at once, the
inner size and its relation to the declared cache, and D070 attributed the result to the wrong
one.

So the clamp is removed. `selfish pack` warns instead, and the warning says that lowering the
declared size was tried and does not help, because that is the obvious next idea for anyone who
hits this. `Builder::cache_size` stays: the override is harmless and a caller may have a reason,
but nothing here uses it, and writing a value no real package carries in exchange for nothing was
the mistake.

### The part that matters more than the field

This was supposed to be the *control* for whether selfish's indirect signature block causes a
console panic. A package needs `payload_blocks > 12` to use one, and a payload block is `0x10000`:

```text
   no indirect   needs   inner <=  720896
   mounts at all needs   inner >  ~851968
```

**Those cannot both hold.** At this block size every package large enough to mount necessarily
uses an indirect block, so "does it still panic without one" is not a question hardware can be
asked. The control is impossible, not merely unbuilt.

That is not a dead end for the investigation, because the indirect block was already cleared by
inspection - `examples/indirect_probe` shows ours and a real package's are the same structure,
and a real package with 944 blocks mounts. What dies here is the *experiment*, and knowing an
experiment cannot exist is worth recording: the next session will otherwise design it again.

