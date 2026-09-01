# `PARAM.SFO` and `param.json`, and eleven files that disagreed with the source


New crate, `selfish-title`: the binary key-value table every package carries, and the JSON the
current generation writes into a title directory. Both read, both written.

A citable source turned up for the first time on the first look - `pfd_sfo_tools`, which is a
**reader and a writer**, plus `ps5upload` as an independent second reading in another
language. Compare that with the package, which still has no writer and is still blocked.

### The three real packages parse

Twenty-nine keys each, and the titles are what they should be:

```
Store-R2-PS5           TITLE_ID NPXS39041   TITLE "Store (PS5)"
PS5_ITEM00001_v1.14    TITLE_ID ITEM00001   TITLE "Itemzflow Game Manager (PS5)"
PS5_LAPY20011_v1.05    TITLE_ID LAPY20011   TITLE "PS5-Xplorer"
```

`PARAM.SFO` is a package *entry* rather than a file inside the filesystem, so extracting a
package does not produce it. `selfish title` takes a package, a bare `.sfo`, or a `param.json`
and works out which by content.

### Then the round trip failed, three times, and each failure was a real fact

`--round-trip` writes the parsed table back and compares byte for byte. Three rounds:

**Four bytes too long.** The cited writer pads the key table so the whole file is a multiple
of sixteen. No real file does - the three packages are 1868 bytes, which is not a multiple of
sixteen at all.

**Three bytes too short.** Removing the padding entirely was also wrong. Six more files were
pulled in - two PS4 toolchain samples and six PS3 titles and saves, nine distinct files, seven
distinct key-table lengths - and the rule fell out cleanly: the key table is padded to a
multiple of **four**, and the file itself is not padded. Seven for seven. (D019)

**One byte too long, on a PS3 save.** `0x0004` is UTF-8 *without* a terminator and the first
version wrote every string as terminated. The difference showed up in a format code fifty-five
bytes in. Now a separate value variant, so a value cannot be held next to a format it
disagrees with. (D020)

**Eleven of eleven now round-trip byte-identical**, across three console generations.

### What that says about the method

None of the three would have been caught by parsing. Two are gaps a reader skips by
construction, and the third produces a string that is correct in every way except its length.
Writing bytes and diffing them is the only thing that finds this class, and it needed *real*
files - the synthetic ones agreed with the wrong rule perfectly.

### `param.json` keeps what it does not understand

Four fields have grounds - `titleId`, `applicationCategoryType`, and `titleName` under the
locale that `defaultLanguage` names. A real file carries dozens more, all store metadata with
no citable meaning here. Modelled as named accessors over the parsed document rather than a
struct: a struct drops the unknown keys, and every real file would come back shorter than it
went in.

The title name is deliberately not one field. `localizedParameters` is a map, and taking the
first entry gives a Japanese title for a title that ships in twelve languages - right shape,
wrong answer, no error.

