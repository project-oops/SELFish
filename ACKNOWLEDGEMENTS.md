# Acknowledgements

Every project consulted to establish a format here, and what was taken from each. No code was
copied from any of them: structure was read from published source, recorded as data in `data/`,
and implemented from that record. Sources are cited by project and commit; `@vendored` marks a
copy taken at an unrecorded point, and an entry without a commit is a citation still to be
completed.

## Container and executable formats

| project | what it gave | how it was used |
|---|---|---|
| **shadPS4**<br/>`shadPS4@be21649` | `src/core/loader/elf.h` - the container header and segment descriptors, as a reader sees them | shape of `self_header` and `self_segment`, and the flag bits |
| **fpPS4** | `sys/sys_types.pas` - the same structures in Pascal, written independently | the second reading; the header constant `00 01 01 12` |
| **OpenOrbis PS4 Toolchain** | `scripts/make_fself.py` - a writer | the extended info block, the control block, metadata, the footer and the entry props layout, which no reader describes |
| **prosperity**<br/>`prosperity@3475257` | `tools/pkg_extract/pkg_extract.py` | the package container, the nesting and the key chain; a further confirmation of the container layout |
| **orbistoun** | `crates/orbistoun-elf/src/wrapper.rs`, `orbistoun#D049` | the current generation's container magic, observed from real material |

## Title metadata

| project | what it gave | how it was used |
|---|---|---|
| **pfd_sfo_tools** (flatz), vendored in `etaHEN` | `sfopatcher/src/sfo.{c,h}` - a `PARAM.SFO` reader and writer | the header, the index entries, the three-table layout and the writing order |
| **ps5upload** | `engine/crates/ps5upload-{core,pkg}` - a reader and a writer in Rust, GPL-3 | the second reading; the format codes; the `localizedParameters` structure of `param.json` |

Both pad a written `PARAM.SFO` to sixteen bytes; eleven real files pad the key table to four and
the file not at all. `data/sfo-format.tsv` records the measured rule beside the cited one.

## Packages

| project | what it gave | how it was used |
|---|---|---|
| **LibProsperoPKG** (SvenGDK)<br/>`LibProsperoPKG@main` | `PFS/ProsperoPs5InnerMetadata.cs` - the inner superblock and inode layout as a current-generation library states them, including the index at `0xD8` | confirmed `0xD8` is a field; its `version = 2`, `mode = 0x18` inner format is not the one the sampled packages use |
| **LibOrbisPkg** (maxton)<br/>`LibOrbisPkg@6434772` | `PKG/Enums.cs` - the entry id names; `PKG/PkgBuilder.cs` - how each entry is produced and the assembly order; `Rif/LicenseDat.cs` - the licence structure; `PFS/PfsStructs.cs` - the superblock, inode and directory-entry layouts; `PFS/PFSBuilder.cs` - block allocation and signature ordering; `PFS/PfsProperties.cs` - that the inner filesystem is plain and the outer one holds one file; `PFS/PFSCWriter.cs` - the container header; `Util/Crypto.cs` - the key derivations; `Util/Keys.cs` - the package moduli, the debug RIF keyset and the licence secret key; `PlayGo/ChunkDat.cs` - `playgo-chunk.dat` | the package entry names and derivations, `PLAYGO_CHUNK_SHA`, every superblock field, the filesystem writer, the licence and the key blobs (D047, D054), and with shadPS4's `playgo_chunk.h` the chunk table (D099). Keys were read from a local clone and checked by reproducing real signatures |
| **PS4-Store** (LightningMods) | `Store/pkg.gp4` and three `sce_sys/param.sfo` files | an oracle, not a source: confirmed the fake passcode and the previous generation's `CATEGORY` values |

The entries `0x1`, `0x100`, `0x80` and `0x1002` were derived from packages and then found under
the same meaning in `PKG/Enums.cs`; `selfish derive` re-checks them (D035).

## Shader container

| project | what it gave | how it was used |
|---|---|---|
| **craziiEmu**<br/>`craziiEmu@8a13647` | `src/CraziiEmu.Libs/Agc/AgcExports.cs` - `sceAgcCreateShader` reading the container: the `0x34333231` magic, the `0x18` version, the pointer table at `0x08`-`0x38`, the type and register-count bytes at `0x5A`/`0x5C`, and `RelocatePointerField` | the self-relative pointer scheme, and the compute program-register offsets `0x20C`/`0x20D` (D103) |
| **prosper**<br/>`prosper@bb3e189` | `src/hle/graphics/hle_agc.cpp` - `struct AgcShader`, with a `static_assert` pinning every offset; cites Kyty | the whole header, including `shader_size` at `0x44` and `target` at `0x4c` (D103) |
| **KytyPS5**<br/>`KytyPS5@0b4e78c` | `src/graphics/shader/shader.h` - `struct Shader` and the sub-table structs; that a code pointer must be 256-aligned | the second full reading, and the sub-table shapes (D103) |
| **Kyty** | `source/emulator/include/Emulator/Graphics/Shader.h:974` - the upstream `struct Shader` | the origin of the layout (D103) |
| **SharpEMU** | `src/SharpEmu.Libs/Agc/AgcExports.cs` - the magic, version and `num_sh_registers` checks | a fifth reader of the identity fields (D103) |

## Name vocabularies, consulted but not held

`examples/name_nid` takes a word list by path, and this repository keeps none (D089).

| project | what it gave | how it was used |
|---|---|---|
| **obSCEne** | `data/mined-names.txt`, `data/nid-corpus.txt`, `data/unnamed-nids.txt` - its own mining output, citing its sources | the vocabulary searched for three unnamed identifiers; nothing was copied |
