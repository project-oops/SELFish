# D101 - A module built here cannot carry a weak undefined import, by construction; the forcing to GLOBAL stays

**Status: decided. 2026-09-10.**


**`mkmodule` forces `STB_GLOBAL` on every resolved import and refuses an unclaimed one, so a module
built through this toolchain cannot carry a weak undefined import. That is kept. The consequence
Orbistoun identified - a weak-symbol absence control cannot survive packaging - is real and is a
property of the toolchain, not an accident of a sample.**

## The question

Orbistoun (REQ-20260910T1105Z-5d20) asked whether `mkmodule` should preserve `STB_WEAK` for an
import whose source carried it, rather than forcing `GLOBAL` on all of them. The tension is
genuine, between two measured facts:

- obSCEne's census control `900-surface/control` uses `obs_census_control_absent`, a deliberately
  non-existent name, to prove the census can tell present from absent. On the **payload leg** (a
  raw ELF) it is `STB_WEAK`, binds to zero per the gABI (orbistoun D676), reads **absent**, and the
  control passes.
- On the **packaged leg** (the same probe through `mkmodule`) every one of its 244 imports is
  `STB_GLOBAL`, including that symbol - so orbistoun stubs it, it reads **present**, and the control
  fails.

Orbistoun framed it as (a) the platform's packager strips weak binding, so all-GLOBAL is faithful;
or (b) the titles measured simply had no weak imports and forcing GLOBAL over-generalises. The
discriminator it proposed: does a genuine title ever emit a `STB_WEAK` dynamic import?

## Why (b) is refuted, without needing a retail title

(b) assumes preserving source binding would be harmless. It would not. obSCEne's probe declares
**every** platform import weak, so that an absent one links as null rather than failing the link -
the dynamic symbol table it hands `mkmodule` is all-`WEAK` undefined. And obscene#D248 measured what
the real loader does with that: a module whose 203 imports were all `WEAK FUNC` had them bound
**only from the two libraries already resident** (`libkernel`, `libSceLibcInternal`); every other
declared library was mapped and left unbound, and fourteen imports stayed null whose symbols the
same process could resolve by name moments later.

So preserving source binding would produce a module whose real imports do not bind - which is the
exact defect the forcing was introduced to fix. Forcing `GLOBAL` is load-bearing and measured, and
it stays. This holds whatever retail titles do, so the retail-title measurement Orbistoun asked for
is not needed to decide it. (I could not supply it regardless: there is no genuine title in this
repository to read, only obSCEne's own output.)

## Why "all-GLOBAL" is structural, not a happenstance of the sample

A weak undefined import cannot reach a finished module through this toolchain, for reasons upstream
of any sample:

- `selfish-elf`'s `rebuild_symbols` **errors** on an undefined symbol it cannot resolve
  (`BuildError::Unclaimed`, `crates/selfish-elf/src/dynlib.rs:575`; test
  `an_unclaimed_import_is_an_error_and_is_named`, `:1313`). An import either resolves - and is then
  set `FUNCTION` + `GLOBAL` (`:556`; test `an_import_is_rebound_global_even_when_the_compiler_marked_it_weak`,
  `:1744`) - or the build fails.
- obSCEne's `mkmodule` **independently refuses** a module whose undefined symbols are not all in its
  `src/imports.c` manifest (`tool/src/unresolved.rs`, `manifest.check_covers`). Its own note records
  that `obs_census_control_absent` had to be added for exactly this reason - the build would not
  proceed without it.

`obs_census_control_absent` is in that manifest (`src/probe/imports.c:406`, under
`libSceLibcInternal`), so `mkmodule` hashes it to a NID and marks it `GLOBAL` like every other
import. Measured: the packaged `PPSA99980/eboot.bin` carries 244 imports, all resolved to libraries
(96 `libkernel`, 86 `libSceLibcInternal`, ...). There is no path by which it could have stayed weak:
leaving it out of the manifest fails both builds; putting it in forces `GLOBAL`.

So Orbistoun's conclusion is correct and its (a) branch is the right resolution - **the packaged
census cannot feature-detect by weak symbol** - but the mechanism is not a packager stripping a
binding. It is the loader refusing to bind weak imports (D248) plus two independent refuse-unclaimed
invariants. A weak undefined import is not *stripped* here; it is *never representable* here.

## The one thing nobody has measured, which is the real hinge

What makes the payload-leg control read "absent" is the gABI rule for an *unresolved weak* symbol:
bind to zero. In the packaged leg the symbol is a `GLOBAL` import carrying a NID that
`libSceLibcInternal` does not export. What a **real loader** does with that - leave it null
(**absent**), or fault the load - is unmeasured. orbistoun's emulator stubs it (**present**), which
is orbistoun's choice and may or may not match hardware.

That is the fact that actually governs whether the packaged leg could ever be a meaningful control,
and it is a hardware question - obSCEne's to answer, not settleable here. This decision does not
depend on it: the forcing stays regardless, because it is demanded by the real imports, not by the
control.

## Consequence, recorded for both sides

- The packaged census control `900-surface/control` is **payload-only by construction**. A
  weak-symbol absence probe cannot survive packaging, because an undefined import is either forced
  `GLOBAL` or rejected.
- If a packaged-leg absence control is wanted, it cannot be an undefined import at all - it would
  need a different mechanism (a defined weak-zero symbol, or detecting absence some other way). That
  is obSCEne's and orbistoun's design space, not a format fact, so it is noted rather than
  prescribed.

No code change: the behaviour is correct and already pinned by the two tests cited above. This entry
is the record of why it cannot be relaxed for a symbol that was *meant* to be absent.
