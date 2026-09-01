# D019 - The `PARAM.SFO` alignment rule came from a source and was refuted by eleven files


The C writer this format was derived from pads the key table so the **whole file** is a
multiple of sixteen. Implemented as cited, it produced files four bytes longer than the real
ones, differing at offset twelve - `data_table_offset`, which is what a loader reads to find
any value at all.

Eleven real files were checked: three PS5 packages, two PS4 toolchain samples, and six PS3
titles and saves. Every one pads the **key table to a multiple of four** and ends exactly
where its last value ends. Seven distinct key-table lengths, and the four-byte rule reproduces
all seven; the sixteen-byte rule reproduces none.

The measured rule is in `data/sfo-format.tsv` and the refuted one is recorded next to it, so
the source cannot be re-read and re-believed. A test states the seven measurements.

This is principle 2 working as intended and it is worth naming: the source was not wrong to
cite, it was wrong about **this generation of material**, and only writing bytes and comparing
them found that out. A reader would never have noticed - the padding is a gap a reader skips.

