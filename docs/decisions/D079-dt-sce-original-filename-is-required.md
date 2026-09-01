# D079 - `DT_SCE_ORIGINAL_FILENAME` is required, and holds the module's own name


Tag `0x6100_0009`. A real executable puts the whole path its build produced there -
`C:/Users/.../ORBIS_Debug/itemz_loader.elf` - which is a filename in the loosest sense and says
more about the machine that built it than about the module.

It was named in this crate's table and emitted by nothing, which was a gap rather than a
decision. **A shared library is refused without it**, and the loader counts what it found:

```text
[rtld] ERROR preprocess_dt_entries:9600: C: orig fn 0  mod info 1
```

One module-info tag, zero of these.

What goes in it is the module's own name, which is already in the string table. Nothing here has
a build path to put - a module is built from sources, not from a file whose name the format
cares about - and a loader needs the tag present and its offset to resolve, not the value to be
a path. Emitting a fabricated path would be a value in a field nothing here can justify.

