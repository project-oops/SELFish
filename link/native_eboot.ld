/*
 * Linker script for a native current-generation (PS5) console executable.
 *
 * Based at offset 0 (ASLR-relocatable, ET_DYN / 0xFE10).
 * No PT_INTERP: the native kernel invokes /libexec/ld-elf.so.1 directly.
 */

PHDRS
{
    text      PT_LOAD FLAGS(5);   /* read + execute */
    data      PT_LOAD FLAGS(6);   /* read + write */
    dynlibdata 0x61000000 FLAGS(4);
    dynamic   PT_DYNAMIC FLAGS(6);
    procparam 0x61000001 FLAGS(4);
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

    .sce_process_param : { KEEP(*(.sce_process_param)) } :data :procparam

    .data           : { *(.data .data.*) }    :data
    .got            : { *(.got) }             :data
    .got.plt        : { *(.got.plt) }         :data
    .bss            : { *(.bss .bss.* COMMON) } :data

    .sce_dynlibdata : { KEEP(*(.sce_dynlibdata)) } :dynlibdata
    .dynamic        : { *(.dynamic); . += 0x20000; } :data :dynamic

    /* .eh_frame is deliberately NOT discarded. libunwind built freestanding
       (_LIBUNWIND_IS_BAREMETAL) finds its frame table by scanning between
       __eh_frame_start and __eh_frame_end rather than through dl_iterate_phdr,
       so discarding the section left both symbols equal and every table lookup
       empty. The visible effect was that a `throw` never reached any `catch`,
       not even `catch (...)`: with no frame table the unwinder cannot walk out
       of the throwing frame at all, so it calls std::terminate instead. The
       loader's "corrupted eh_frame_hdr or eh_frame" warning is the same fact
       reported one layer down. Costs ~19 KiB on a title that uses exceptions
       and nothing on one that does not. */

    /DISCARD/ : {
        *(.comment)
        *(.note .note.*)
        *(.gnu.version*)
    }
}
