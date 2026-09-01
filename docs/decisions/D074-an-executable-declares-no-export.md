# D074 - An executable declares no export library, which frees library id zero


Status: **decided**, 2026-08-29.

Export and import libraries share one id space. A shared library takes id zero for the library
it exports and numbers its imports from one; that is the ordinary arrangement, and `entries`
wrote it unconditionally.

A main executable exports nothing, so its first import library **is** id zero. Emitting the
export tag on one costs that slot and pushes every import library up by one, leaving the table
a loader indexes with no entry at the front.

Settled against a launching homebrew executable, whose numbers are unambiguous: ten import
libraries at ids 0 through 9, nine needed modules at ids 1 through 9, no export tag of either
kind, and symbol suffixes to match. `Segment::entries` now takes the object type and the module
itself answers it - `install` reads `e_type` from the file it is editing rather than taking a
caller's word for what it is building.

**This was not the failure being chased**, and that is worth recording. It was found while
looking for the cause of a refusal that turned out to be three layers lower down (D075), and it
is right on its own evidence rather than because fixing it fixed anything.

