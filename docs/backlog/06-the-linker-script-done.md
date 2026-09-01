# 6. ~~The linker script~~ - done


`link/module.ld`, with `selfish-elf::layout` holding the constants it encodes and asserting
the two agree. An integration test links a real object with `ld.lld` and reads the result back
through this crate; it skips when the toolchain is absent. (D024)

