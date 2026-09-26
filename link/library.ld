/*
 * Linker script for a shared library a title bundles in `sce_module`.
 *
 * The constants it names are also declared in `selfish-elf::segment`, and a test asserts the
 * two agree, because no compiler checks a linker script. The canonical copy lives here (D024).
 *
 * A script declares segments and places sections; encoded symbol names come from generated
 * stub libraries. lld accepts numeric segment types, so the vendor segments are declared at
 * link time: 0x61000000 PT_SCE_DYNLIBDATA, 0x61000002 the module parameters.
 */

PHDRS
{
    /* Two loadable segments, read-execute and read-write, as in the reference module; its
     * read-only region is PT_SCE_RELRO, not a PT_LOAD. A loader that maps segments by kind
     * drops a third loadable segment, leaving `.rodata` unmapped.
     *
     * No FILEHDR or PHDRS, unlike `module.ld`. The loader reads a bundled library's headers
     * from the file, and refuses one whose first segment starts underneath them. */
    text      PT_LOAD FLAGS(5);   /* read + execute; headers outside every segment */
    data      PT_LOAD FLAGS(6);   /* read + write */
    modparam  0x61000002 FLAGS(4);
    dynlibdata 0x61000000 FLAGS(4);
    dynamic   PT_DYNAMIC FLAGS(6);
}

SECTIONS
{
    /* Based at zero with the headers outside the first segment, the combination a bundled
     * library needs. Page alignment to the hardware's 16 KiB granularity pushes the first
     * segment to file offset 0x4000 at virtual address zero, as in the vendor's libraries.
     * A first segment at file offset zero is refused by the system loader's header check:
     *
     *     [rtld] ERROR verify_ehdr:930: B: offset 0x0  end 0x190
     *     [rtld] ERROR self_load_shared_object:2816: Unsupported ELF e_type. libc.prx fe18
     *
     * `0x190` is the end of the program header table. The second line follows the failed
     * check; `0xfe18` is the correct `e_type` for a bundled library. */
    . = 0;

    .text           : { *(.text .text.*) }    :text
    .plt            : { *(.plt .plt.*) }      :text

    . = ALIGN(0x4000);
    .sce_module_param : {
        *(.data.sce_module_param)
        *(.data.sce_module_param.*)
    } :modparam :data
    .rodata         : { *(.rodata .rodata.*) } :data
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

    .hash           : { *(.hash) }            :data
    .gnu.hash       : { *(.gnu.hash) }        :data
    .dynsym         : { *(.dynsym) }          :data
    .dynstr         : { *(.dynstr) }          :data
    .rela.dyn       : { *(.rela.dyn) }        :data
    .rela.plt       : { *(.rela.plt) }        :data
    .sce_dynlibdata : { KEEP(*(.sce_dynlibdata)) } :dynlibdata

    /* None of these belongs in a loaded module. */
    /DISCARD/ : {
        *(.comment)
        *(.note .note.*)
        *(.eh_frame .eh_frame_hdr)
        *(.gnu.version*)
    }
}
