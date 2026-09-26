/*
 * Linker script for a module that a homebrew loader or an emulator maps.
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
    /* Two loadable segments, read-execute and read-write, as in the reference module; its
     * read-only region is PT_SCE_RELRO, not a PT_LOAD. A loader that maps segments by kind
     * drops a third loadable segment, leaving `.rodata` unmapped.
     *
     * FILEHDR and PHDRS put the headers inside the first segment, so a loader that maps
     * only what segments describe can read the program header table. */
    text      PT_LOAD FILEHDR PHDRS FLAGS(5);   /* read + execute */
    data      PT_LOAD FLAGS(6);   /* read + write */
    /* Declared here, filled in after linking: the builder appends the string table,
     * NID-named symbol table and hash table, then rewrites this header's offset and size. */
    dynlibdata 0x61000000 FLAGS(4);
    dynamic   PT_DYNAMIC FLAGS(6);
    /* Not loadable, so not a third mapping. An explicit PHDRS block means lld emits only
     * the segments listed, so this header exists only because it is declared here (D098).
     * The interpreter string is in the consumer's runtime. */
    interp    PT_INTERP FLAGS(4);
    procparam 0x61000001 FLAGS(4);
}

SECTIONS
{
    /* Addresses here are offsets; the loader adds a base at load time. A system loader maps
     * at the stated addresses and refuses this layout, so an eboot uses `link/eboot.ld`,
     * which also differs in its PHDRS block. Segments start on the hardware's 16 KiB
     * allocation granularity, not the host's 4 KiB: a segment starting part-way into a page
     * faults inside the loader. */
    . = SIZEOF_HEADERS;

    /* In both segments: PT_INTERP names a location the loader reads, so the bytes must be
     * inside a LOAD segment, and `:interp` gives the segment header that points at them.
     * First in the first segment, as in the reference module. The link tables that follow
     * share the executable segment with the code. */
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
    /* Read-only data shares the executable segment, as in the reference module. */
    .rodata         : { *(.rodata .rodata.*) } :text

    . = ALIGN(0x4000);
    /* The consumer's runtime defines the process-parameter structure; its own segment is how
     * the loader finds it. */
    .sce_process_param : { KEEP(*(.sce_process_param)) } :data :procparam

    .data           : { *(.data .data.*) }    :data
    /* Two sections, not one. A linkage stub indexes from the start of `.got.plt`, and the
     * loader writes resolved imports relative to DT_SCE_PLTGOT; merged, PLTGOT would name
     * the start of `.got` and every resolved address would land at the wrong offset. The
     * tool's fallback to `.got` is for modules with no linkage table. */
    .got            : { *(.got) }             :data
    .got.plt        : { *(.got.plt) }         :data
    .bss            : { *(.bss .bss.* COMMON) } :data

    /* The builder rewrites the dynamic table after linking with vendor tags the linker does
     * not know, so the space is reserved here. Each imported library costs four tags
     * (`DT_NEEDED`, `DT_SCE_NEEDED_MODULE`, `DT_SCE_IMPORT_LIB`, `DT_SCE_IMPORT_LIB_ATTR`) on
     * top of the eighteen describing the tables, at sixteen bytes a tag:
     *
     *     16 libraries    82 tags     1,312 bytes   fits in 0x800
     *    556 libraries  2,242 tags    35,872 bytes  does not
     *
     * 0x20000 holds about 8,000 tags, roughly 2,000 libraries. A table that does not fit is
     * a build error reported by the builder, never written over what follows. */
    .dynamic        : { *(.dynamic); . += 0x20000; } :data :dynamic

    /* A placeholder so the segment exists in the header table: lld drops a PHDRS entry with
     * no section assigned to it. */
    .sce_dynlibdata : { KEEP(*(.sce_dynlibdata)) } :dynlibdata

    /* None of these belongs in a loaded module. */
    /DISCARD/ : {
        *(.comment)
        *(.note .note.*)
        *(.eh_frame .eh_frame_hdr)
        *(.gnu.version*)
    }
}
