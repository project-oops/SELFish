# D082 - A package cannot produce a current-generation title, so `native` is a second delivery route rather than a second package format


**A package cannot produce a current-generation title, so `native` is a second delivery route
rather than a second package format.**

The question was whether a package could be made to install as a current-generation title -
whether the badge could be changed, or a package could be made to call the install API itself.
Neither works, and the reasons are worth recording so this is not re-attempted:

- **There is no package format to build.** The previous generation's fake-package scene exists
  because its keyset is public - it is in `data/pkg-keys.toml`. No equivalent exists for this
  generation, so there is nothing to sign a native package with.
- **A package cannot install itself out of the sandbox.** The registration call is
  `sceAppInstUtilAppInstallTitleDir`, and the shipping native homebrew reaches it through
  `kernel_dynlib_handle` / `kernel_dynlib_resolve` - kernel read/write primitives. Code inside
  a compatibility-sandboxed application does not have them, so a package attempting this fails
  at the resolve, not at the call.
- **The route that works needs no package.** A payload with kernel privileges writes
  `/user/app/<TITLE_ID>/sce_sys/{param.json,icon0.png}` and registers it.

So `selfish native` and obSCEne's `make native` lay out that directory, and **nothing installs
anything** - the copy and the call need privileges a build machine does not have.

**What this does not buy, stated because it is the tempting misreading.** It is a home-screen
entry, not a title that runs native code. That would need a signed `eboot.bin`. The evidence is
in the shipping native homebrew itself: it installs a `param.json` and an `icon0.png` and **no
executable**, and its `param.json` carries a `deeplinkUri` pointing at a server its payload
runs. obSCEne's own code already runs outside the compatibility sandbox - as a payload. This
target gives that payload somewhere to be launched from.

The NID was confirmed with this project's own tooling: `selfish nid
sceAppInstUtilAppInstallTitleDir` gives `Wudg3Xe3heE`, which is the identifier the shipping
homebrew resolves.

