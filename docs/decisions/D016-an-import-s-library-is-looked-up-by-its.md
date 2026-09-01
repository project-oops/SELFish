# D016 - An import's library is looked up by its id, not by its position in the table


The import-library table is a list of packed `(id, name-offset)` values, and the obvious
reading is that entry *n* is library *n*. Real material says otherwise.

A vendor `libc.prx` lists its libraries in the order **1, 2, 3, 0** - `libSceFios2`,
`libSceLibcInternalExt`, `libSceSysmodule`, then `libkernel` last and numbered zero. Ninety-six
of its hundred and nine imports are from library zero. Indexing by position attributes every
one of them to `libSceFios2`.

That failure has no symptom. The output is a real library name, a plausible count, and a
completely wrong attribution - the same shape as the mistake the two vendor tag ranges caused
(D013), which is why the lookup is by id and a test states the ordering that proves it.

The same table drove a second confirmation worth recording: the store's `eboot.bin` imports
eight symbols from library `libScePosix` in module `libkernel`. Library and module are
genuinely different namespaces, and a reader that collapses them loses the distinction on the
first module that uses it.

