/*
 * Linker script for a native Prospero-generation executable.
 *
 * Based at offset 0 (ASLR-relocatable, ET_DYN / 0xFE10).
 * No PT_INTERP: the kernel invokes /libexec/ld-elf.so.1 directly.
 */

PHDRS
{
    text      PT_LOAD FLAGS(5);   /* read + execute */
    data      PT_LOAD FLAGS(6);   /* read + write */
    dynlibdata 0x61000000 FLAGS(4);
    dynamic   PT_DYNAMIC FLAGS(6);
    procparam 0x61000001 FLAGS(4);
    tls       PT_TLS FLAGS(4);
}

SECTIONS
{
    . = 0;

    .text           : { *(.text .text.*) }    :text
    .plt            : { *(.plt .plt.*) }      :text

    . = ALIGN(0x4000);
    .hash           : { *(.hash) }            :data
    .gnu.hash       : { *(.gnu.hash) }        :data
    .dynsym         : { *(.dynsym) }          :data
    .dynstr         : { *(.dynstr) }          :data
    .rela.dyn       : { *(.rela.dyn) }        :data
    .rela.plt       : { *(.rela.plt) }        :data
    .rodata         : { *(.rodata .rodata.*) } :data

    /* C++ dynamic-initialiser tables, run by the consumer's runtime through the bounds symbols. */
    . = ALIGN(8);
    .preinit_array  : {
        PROVIDE_HIDDEN(__preinit_array_start = .);
        KEEP(*(.preinit_array))
        PROVIDE_HIDDEN(__preinit_array_end = .);
    } :data
    .init_array     : {
        PROVIDE_HIDDEN(__init_array_start = .);
        KEEP(*(SORT_BY_INIT_PRIORITY(.init_array.*) SORT_BY_INIT_PRIORITY(.ctors.*)))
        KEEP(*(.init_array .ctors))
        PROVIDE_HIDDEN(__init_array_end = .);
    } :data
    .fini_array     : {
        PROVIDE_HIDDEN(__fini_array_start = .);
        KEEP(*(SORT_BY_INIT_PRIORITY(.fini_array.*) SORT_BY_INIT_PRIORITY(.dtors.*)))
        KEEP(*(.fini_array .dtors))
        PROVIDE_HIDDEN(__fini_array_end = .);
    } :data

    .sce_process_param : { KEEP(*(.sce_process_param)) } :data :procparam

    .tdata          : { *(.tdata .tdata.*) }   :data :tls
    .tbss           : { *(.tbss .tbss.* .tcommon) } :data :tls

    .data           : { *(.data .data.*) }    :data
    .got            : { *(.got) }             :data
    .got.plt        : { *(.got.plt) }         :data

    .sce_dynlibdata : { KEEP(*(.sce_dynlibdata)) } :dynlibdata
    .dynamic        : { *(.dynamic); . += 0x20000; } :data :dynamic

    /* .bss is last in the :data segment so it stays NOBITS and no zero padding is written
       into the file before a later PROGBITS section. */
    .bss            : { *(.bss .bss.* COMMON) } :data

    /* .eh_frame is kept. Freestanding libunwind (_LIBUNWIND_IS_BAREMETAL) finds its
       frame table between __eh_frame_start and __eh_frame_end rather than through
       dl_iterate_phdr; without it no `throw` reaches a `catch` and std::terminate
       runs. About 19 KiB on a title that uses exceptions, nothing otherwise. */

    /DISCARD/ : {
        *(.comment)
        *(.note .note.*)
        *(.gnu.version*)
    }
}
