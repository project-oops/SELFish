# D081 - `param.json` does not belong in a package, and its presence there was mixing two routes


**`param.json` does not belong in a package, and its presence there was mixing two routes.**

The package tree carried `sce_sys/param.json`. No real package does: three extracted samples
have exactly one file in `sce_sys`, and it is the keystone. A package's title metadata is the
`param.sfo` **entry**, which is generated.

`param.json` is not wrong, it belongs to the other delivery route entirely - a **native title
directory** at `/user/app/<TITLE_ID>/sce_sys/param.json`. Writing one inside a package got the
benefit of neither. It is now removed from the package tree, and D082 gives it its proper home.

