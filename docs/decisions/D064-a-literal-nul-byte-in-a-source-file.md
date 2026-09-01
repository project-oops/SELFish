# D064 - A literal NUL byte in a source file made it invisible to `grep`, which hid a duplicate constant


`crates/selfish-pkg/src/write.rs` contained a raw `0x00` inside a byte-string literal -
`b"RIF<NUL>"` rather than `b"RIF\0"`. It compiles and the test passed, so nothing complained.

But git and `grep` classify a file containing NUL as **binary and skip it silently**. Every
search run across this repository was quietly missing that file, which is how the duplicated
`IMAGE_KEY_INDEX` survived: the search that would have found it could not see the file it was
in. `examples/decrypt.rs` had the same byte.

Worth a decision entry because the lesson is not "avoid NUL bytes" - it is that **a tool
reporting nothing is not the same as a tool reporting no matches**, and `grep` says nothing
when it skips a binary file. Both are now the escape.

