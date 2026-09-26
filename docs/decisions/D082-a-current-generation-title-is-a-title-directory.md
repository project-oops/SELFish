# D082 - A current-generation title is a title directory

**Status:** decided
**Date:** 2026-09-26

`--format title` lays out `<output>/<TITLE_ID>/` with `sce_sys/param.json` and artwork, the
shape `sceAppInstUtilAppInstallTitleDir` installs. A package (`--format pkg`) is the
compatibility route and carries `param.sfo` as an entry, not `param.json`. Nothing here installs
anything.

**Why:** no public keyset exists for a current-generation package, and a sandboxed package
cannot call the install API itself. The route that works is a title directory registered by
code with the privilege to do so.

**Rejected:**
- A current-generation package format: nothing to sign it with.
- `param.json` inside a package: no real package carries one; it belongs to the other route.
