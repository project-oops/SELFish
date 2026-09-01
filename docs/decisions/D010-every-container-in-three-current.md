# D010 - Every container in three current-generation packages uses the *previous* generation's magic


Measured, by parsing every file in all three samples:

```text
Store-R2-PS5          0 current generation, 9 previous, 28 neither
PS5_ITEM00001_v1.14   0 current generation, 11 previous, 60 neither
PS5_LAPY20011_v1.05   0 current generation, 13 previous, 13 neither
```

Thirty-three containers. Not one carries `54 14 F5 EE`. And `Store-R2-PS5` is a working
homebrew store for the current console, so a previous-generation container demonstrably loads
on current hardware.

### This refines D002 rather than contradicting it

orbistoun observed the current-generation magic on real material and recorded that both
generations coexist inside one title - bundled modules in the new format, substituted stub
libraries in the old. That observation stands; these samples are simply different material.
**Retail uses the current format. Homebrew, built with community tooling, emits the previous
one - and the loader takes it.**

So the generation split is real and both values matter, but the guidance for anything this
project builds is now the opposite of what it looked like an hour ago: **a homebrew container
should carry the previous generation's magic**, because that is the configuration with
evidence behind it.

`Generation` keeps both, keeps no `Default`, and callers keep having to say which - the type
was right even while the recommendation was wrong.

Status: **decided** for the measurement. Which magic a *fake* container should carry on
current hardware is now evidenced rather than inferred, and the evidence points at
`Previous`.

