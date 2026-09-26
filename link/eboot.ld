/*
 * Linker script for an executable loaded through an interpreter.
 *
 * The constants it names are also declared in `selfish-elf::segment`, and a test asserts the
 * two agree, because no compiler checks a linker script. The canonical copy lives here (D024).
 *
 * A script declares segments and places sections; it does not generate content. The
 * process-parameter structure comes from the consumer's runtime and encoded symbol names
 * from generated stub libraries. lld accepts numeric segment types, so the vendor segments
 * are declared at link time: 0x61000001 PT_SCE_PROC_PARAM, 0x61000000 PT_SCE_DYNLIBDATA.
 */

PHDRS
{
    /* Two loadable segments, text and data. A loader refuses a module with more
     * ("Attempting to add too many segments"), and vendor modules keep code, link tables
     * and read-only data together in the first. */
    text      PT_LOAD FLAGS(5);   /* read + execute; headers outside every segment */
    data      PT_LOAD FLAGS(6);   /* read + write */
    dynlibdata 0x61000000 FLAGS(4);
    dynamic   PT_DYNAMIC FLAGS(6);
    interp    PT_INTERP FLAGS(4);
    procparam 0x61000001 FLAGS(4);
    tls       PT_TLS FLAGS(4);
}

SECTIONS
{
    /* Based where the system loader maps an executable, not at zero. */
    . = 0x400000;

    /* In both segments: PT_INTERP names a location the loader reads, so the bytes must be
     * inside a LOAD segment, and `:interp` gives the segment header that points at them.
     * First in the first segment, as in the reference module. */
    .interp         : { KEEP(*(.interp)) }    :text :interp

    .hash           : { *(.hash) }            :text
    .gnu.hash       : { *(.gnu.hash) }        :text
    .dynsym         : { *(.dynsym) }          :text
    .dynstr         : { *(.dynstr) }          :text
    .rela.dyn       : { *(.rela.dyn) }        :text
    .rela.plt       : { *(.rela.plt) }        :text

    . = ALIGN(0x4000);
    .text           : { *(.text .text.*) }    :text
    .plt            : { *(.plt .plt.*) }      :text

    . = ALIGN(0x4000);
    /* Read-only data shares the executable segment with the code and link tables; a third
     * PT_LOAD is one a loader declines to map. */
    .rodata         : { *(.rodata .rodata.*) } :text

    . = ALIGN(0x4000);
    /* The consumer's runtime defines the process-parameter structure; its own segment is how
     * the loader finds it. */
    .sce_process_param : { KEEP(*(.sce_process_param)) } :data :procparam

    .tdata          : { *(.tdata .tdata.*) }   :data :tls
    .tbss           : { *(.tbss .tbss.* .tcommon) } :data :tls

    .data           : { *(.data .data.*) }    :data
    /* Two sections, not one. A linkage stub indexes from the start of `.got.plt`, and the
     * loader writes resolved imports relative to DT_SCE_PLTGOT; merged, PLTGOT would name
     * the start of `.got` and every resolved address would land at the wrong offset. The
     * tool's fallback to `.got` is for modules with no linkage table. */
    .got            : { *(.got) }             :data
    .got.plt        : { *(.got.plt) }         :data

    /* Sized for the number of imported libraries. Each costs four tags (`DT_NEEDED`,
     * `DT_SCE_NEEDED_MODULE`, `DT_SCE_IMPORT_LIB`, `DT_SCE_IMPORT_LIB_ATTR`) on top of the
     * eighteen describing the tables, at sixteen bytes a tag:
     *
     *     16 libraries    82 tags     1,312 bytes   fits in 0x800
     *    556 libraries  2,242 tags    35,872 bytes  does not
     */
    .sce_dynlibdata : { KEEP(*(.sce_dynlibdata)) } :dynlibdata
    .dynamic        : { *(.dynamic); . += 0x20000; } :data :dynamic

    /* .bss is last in the :data segment so it stays NOBITS and no zero padding is written
       into the file before a later PROGBITS section. */
    .bss            : { *(.bss .bss.* COMMON) } :data

    /* None of these belongs in a loaded module. */
    /DISCARD/ : {
        *(.comment)
        *(.note .note.*)
        *(.eh_frame .eh_frame_hdr)
        *(.gnu.version*)
    }
}
