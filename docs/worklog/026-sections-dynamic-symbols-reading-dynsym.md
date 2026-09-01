# `Sections::dynamic_symbols` - reading `.dynsym`


`symbols()` read only `.symtab`; a stripped shared object keeps only `.dynsym`, so reading a
payload's imports needed the dynamic table. Added `dynamic_symbols()` beside `symbols()`, both
over a shared `symbols_of(kind)`. Pure format knowledge - the kind of gap this repository exists
to fill once rather than in each consumer.

A payload **crt0** was briefly added under `runtime/` and removed: it executes on the console,
which the admission test excludes ("a `crt` ... do not [belong here]"). It lives in obSCEne,
which consumes `dynamic_symbols` + `Nid` to build its resolution table. The boundary held.

