# 2. ~~Section headers and symbol tables~~ - done


`selfish-elf::section` reads section headers, the name table, section contents, and `.symtab`
with its strings. Cross-checked against `readelf` on a real linked object: 14,667 entries and
74 undefined, exactly.

`defines(name)` is the thing a builder actually needs - whether the object *defines* a symbol,
which is what decides whether an initialiser tag belongs in the dynamic table. (D023)

