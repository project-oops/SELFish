/*
 * Linker script for a console module.
 *
 * ---------------------------------------------------------------------------------------
 * Moved here from obSCEne, which wrote it and paid for every comment in it. This is now the
 * canonical copy; see D024. The constants it names are also declared in
 * `selfish-elf::segment`, and a test asserts the two agree, because a linker script is the
 * one artefact in this repository a compiler cannot check.
 *
 * References to a specific build tool below have been generalised to "the builder": the
 * division of labour they describe is real and is the point, but the tool that does it is a
 * consumer's business rather than this file's.
 * ---------------------------------------------------------------------------------------
 *
 * Replaces a growing pile of post-processing. Every vendor requirement found so far was
 * met by rewriting the linked binary afterwards — repurposing a spare program header,
 * overwriting the dynamic table, hunting alignment flags — and each individual fix
 * looked smaller than writing this did. Together they were larger, and they were
 * rebuilding a linker script in Rust, in instalments.
 *
 * A script declares segments and places sections. It cannot *generate* content: the
 * process-parameter structure comes from the consumer's runtime, and encoded symbol names will come from
 * generated stub libraries. Those are the two jobs a script correctly refuses to do.
 *
 * # PHDRS is the whole point
 *
 * lld accepts arbitrary numeric segment types, which is what makes the vendor segments
 * declarable at link time rather than patched in afterwards:
 *
 *   0x61000001  PT_SCE_PROC_PARAM   process parameters
 *   0x61000000  PT_SCE_DYNLIBDATA   the tables a loader actually reads
 *
 * FILEHDR and PHDRS on the first segment are required: without them the headers are not
 * covered by any segment, and a loader that maps only what segments describe cannot read
 * the program header table it just used.
 */

PHDRS
{
    /* Three loadable segments, and not four.
     *
     * A separate read-only segment for the headers and link tables was the obvious
     * layout and it is one too many: a loader refused the fourth outright
     * ("Attempting to add too many segments"). Vendor modules have exactly text,
     * rodata and data, with the ELF header and program header table at the front of
     * the first one - so that is what we emit.
     *
     * FILEHDR and PHDRS here rather than on a segment of their own: without them the
     * headers are not covered by any segment, and a loader that maps only what the
     * segments describe cannot read the header table it just used. */
    text      PT_LOAD FLAGS(5);   /* read + execute; headers deliberately outside every segment */
    data      PT_LOAD FLAGS(6);   /* read + write */
    dynlibdata 0x61000000 FLAGS(4);
    dynamic   PT_DYNAMIC FLAGS(6);
    interp    PT_INTERP FLAGS(4);
    procparam 0x61000001 FLAGS(4);
}

SECTIONS
{
    /* Based where a system loader maps it, not at zero. See the header of this file. */
    . = 0x400000;

    /* First, and in both segments deliberately.
     *
     * `:text` puts the bytes inside a mapped segment, which PT_INTERP requires - it names
     * a location a loader reads, and a location outside every LOAD segment is not there
     * at runtime. `:interp` is what gives the segment its own header pointing at it.
     *
     * Placed at the front to match the module the control experiment proved works, where
     * the interpreter string is the first thing in the first segment. */
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
    /* In the executable segment, with the code and the link tables.
     *
     * Read-only data in an executable mapping is harmless - the tables above are already
     * there for the same reason - and it is what the control build does. What is not
     * harmless is a third PT_LOAD, which one loader silently declines to map at all. */
    .rodata         : { *(.rodata .rodata.*) } :text

    . = ALIGN(0x4000);
    /* The process-parameter structure. the consumer's runtime defines it and this places it in its own
     * segment, which is the only way a loader finds it. */
    .sce_process_param : { KEEP(*(.sce_process_param)) } :data :procparam

    .data           : { *(.data .data.*) }    :data
    /* Two sections, not one.
     *
     * Merging them looked tidy and pointed DT_SCE_PLTGOT at the wrong address. A
     * procedure-linkage stub indexes from the start of `.got.plt`, and a loader
     * writing a resolved import uses PLTGOT as that base - so with the two merged,
     * PLTGOT names the start of `.got` and every resolved address lands at the wrong
     * offset. The symptom is a module that resolves nothing and faults on its first
     * call, which reads as the loader not supporting the format.
     *
     * The tool looks for `.got.plt` and falls back to `.got`; that fallback is for
     * modules with no linkage table at all, not for this.
     *
     * Splitting them did *not* on its own make the PLT build work — one loader still
     * resolves nothing with it. So this is a real defect corrected, and not the reason
     * that build fails. Kept because a wrong PLTGOT is wrong whether or not anything
     * currently depends on it. */
    .got            : { *(.got) }             :data
    .got.plt        : { *(.got.plt) }         :data
    .bss            : { *(.bss .bss.* COMMON) } :data

    /* Sized for the number of libraries, which is what actually drives it.
     *
     * Every imported library costs four tags - `DT_NEEDED`, `DT_SCE_NEEDED_MODULE`,
     * `DT_SCE_IMPORT_LIB`, `DT_SCE_IMPORT_LIB_ATTR` - on top of the eighteen describing the
     * tables. At sixteen bytes a tag:
     *
     *     16 libraries    82 tags     1,312 bytes   fits in 0x800
     *    556 libraries  2,242 tags    35,872 bytes  does not
     */
    .sce_dynlibdata : { KEEP(*(.sce_dynlibdata)) } :dynlibdata
    .dynamic        : { *(.dynamic); . += 0x20000; } :data :dynamic

    /* Nothing here belongs in a loaded module, and leaving them in makes the file
     * larger and the segment layout harder to read. */
    /DISCARD/ : {
        *(.comment)
        *(.note .note.*)
        *(.eh_frame .eh_frame_hdr)
        *(.gnu.version*)
    }
}
