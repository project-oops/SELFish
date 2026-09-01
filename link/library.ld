/*
 * Linker script for a shared library a title bundles in `sce_module`.
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
 * A script declares segments and places sections. It cannot *generate* content: encoded symbol
 * names come from generated stub libraries, which is a job a script correctly refuses to do.
 *
 * # PHDRS is the whole point
 *
 * lld accepts arbitrary numeric segment types, which is what makes the vendor segments
 * declarable at link time rather than patched in afterwards:
 *
 *   0x61000000  PT_SCE_DYNLIBDATA   the tables a loader actually reads
 *
 * **FILEHDR and PHDRS are deliberately absent here**, which is the opposite of `module.ld`
 * and the reason this file exists. A system loader checks the header region before it will
 * load a bundled library at all and refuses one whose first segment starts underneath the
 * program header table. See the base address below.
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
     * **No FILEHDR or PHDRS**, unlike `module.ld`. A loader that maps only what the segments
     * describe would then be unable to read the header table it just used - which is the
     * argument for putting them in, and it does not apply to a bundled library, because the
     * loader reading this one has the file open and reads the headers from it directly. What
     * does apply is the header check: with the headers inside the first segment that segment
     * starts at file offset zero, underneath them, and the file is refused. */
    text      PT_LOAD FLAGS(5);   /* read + execute; headers deliberately outside every segment */
    /* No separate read-only segment, and this was a real bug rather than tidying.
     *
     * There used to be one, on the reasoning recorded above that "vendor modules have
     * exactly text, rodata and data". The control build (D097) says otherwise: an
     * OpenOrbis module has **two** PT_LOAD segments - read-execute and read-write - and
     * puts its read-only data in the first. The read-only region in the middle is
     * PT_SCE_RELRO, which is not a PT_LOAD at all.
     *
     * The cost was total. fpPS4 maps segments by kind, and with three it listed only two:
     *
     *   .CODE:0000000800000000..0000000800029000
     *   .DATA:0000000800038000..0000000800040000
     *
     * The read-only segment between them is simply absent - so every `.rodata` byte was
     * unmapped, which is where the section tables, the check tables and every string in
     * the report live. `obs_run_all` walks `obs_sections[]` as its first act and read
     * unmapped memory, which is the EAccessViolation immediately after `Entry:`.
     *
     * A minimal module survived because it is small enough to have no separate read-only
     * segment to lose, which is why the fault looked like a problem of scale. */
    data      PT_LOAD FLAGS(6);   /* read + write */
    dynamic   PT_DYNAMIC FLAGS(6);
    /* **No `PT_INTERP` and no process parameters**, unlike the two executable scripts.
     *
     * A library is loaded by an executable that already has both. It has neither an
     * interpreter of its own nor a process to parametrise, and a real bundled library carries
     * neither header - where it does carry one the other two do not, `PT_SCE_MODULE_PARAM`
     * (`0x61000002`), which describes the module rather than the process.
     *
     * Left in, they appear as headers of size zero: an interpreter whose path is the empty
     * string, and process parameters that are not there. Declaring a structure and then
     * declaring it empty is worse than not declaring it, which is the same argument the
     * process-parameter block itself lost in obSCEne D219. (D078) */
    /* Declared here but filled in afterwards. The script can give the segment a header
     * with the right type; it cannot build a string table, a symbol table with
     * NID-encoded names, or a hash table, so the builder appends
     * the contents and rewrites this header's offset and size. */
    dynlibdata 0x61000000 FLAGS(4);
}

SECTIONS
{
    /* 16 KiB is the console's allocation granularity, not the host's 4 KiB. Segments
     * must start on it: a loader mapping whole pages cannot place one that begins
     * part-way into a page, and the symptom is a fault inside the loader rather than a
     * rejection. */
    /* Based at zero, and the headers are still outside the first segment.
     *
     * This is the combination neither of the other two scripts has, and it is what a bundled
     * library needs. `module.ld` is based at zero but covers the headers with the first
     * segment (`FILEHDR PHDRS`); `eboot.ld` leaves them outside but bases at `0x400000`.
     *
     * A system loader verifies the header region before it will load a `.prx` at all, and
     * refuses one whose first segment starts underneath it:
     *
     *     [rtld] ERROR verify_ehdr:930: B: offset 0x0  end 0x190
     *     [rtld] ERROR self_load_shared_object:2816: Unsupported ELF e_type. libc.prx fe18
     *
     * `0x190` is where the program header table ends and `0x0` is where the first segment
     * began. The second line names `e_type` and is a red herring - the value is `0xfe18`,
     * which is what a real bundled library carries; it is the message the loader prints after
     * the header check has already failed.
     *
     * A real one starts its first segment at file offset `0x4000` with virtual address zero,
     * which is what falls out of basing here and letting the page alignment push the file
     * offset past the headers. (D078) */
    . = 0;

    /* The link tables share the first segment with the code. They are read-only data
     * in an executable segment, which is harmless, and it is what keeps the count at
     * three. */
    /* No interpreter section: the PHDRS block above declares no PT_INTERP for a library,
     * and a section with no segment to sit in is a section lld places nowhere useful. */

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

    /* The dynamic table is rewritten after linking, and the vendor table is larger
     * than the standard one the linker produced: three entries per imported library,
     * on top of the fourteen describing the tables. There is no way to tell the
     * linker to emit entries it knows nothing about, so the space is reserved here
     * and filled in afterwards.
     *
     * A table that does not fit must be caught and reported by the builder rather
     * than written over whatever follows. This only has to be large enough. */
    /* Sized for the number of libraries, which is what actually drives it.
     *
     * Every imported library costs four tags - `DT_NEEDED`, `DT_SCE_NEEDED_MODULE`,
     * `DT_SCE_IMPORT_LIB`, `DT_SCE_IMPORT_LIB_ATTR` - on top of the eighteen describing the
     * tables. At sixteen bytes a tag:
     *
     *     16 libraries    82 tags     1,312 bytes   fits in 0x800
     *    556 libraries  2,242 tags    35,872 bytes  does not
     *
     * 0x20000 holds about 8,000 tags, so roughly 2,000 libraries. The padding is zeroes in
     * the writable segment and costs nothing a module of this size will notice.
     *
     * A table that does not fit must be caught and reported by the builder rather than
     * written over whatever follows, so getting this wrong is a build failure and not a
     * mystery. */
    .dynamic        : { *(.dynamic); . += 0x20000; } :data :dynamic

    /* A placeholder so the segment exists in the header table. lld drops a PHDRS entry
     * with no section assigned to it, and a header that is absent cannot be pointed at
     * a table later. */
    .sce_dynlibdata : { KEEP(*(.sce_dynlibdata)) } :dynlibdata

    /* Nothing here belongs in a loaded module, and leaving them in makes the file
     * larger and the segment layout harder to read. */
    /DISCARD/ : {
        *(.comment)
        *(.note .note.*)
        *(.eh_frame .eh_frame_hdr)
        *(.gnu.version*)
    }
}
