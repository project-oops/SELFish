# Acknowledgements

Every project consulted to establish a format here, with what was taken from each. Recorded
because a format claim is only worth what its provenance is worth, and because the question
"where did this come from" should be answerable years later by someone who was not here.

**Reference only. No code was copied from any of these.** Structure was read from published
source, recorded as data in `data/`, and implemented from that record.

## Container and executable formats

| project | what it gave | how it was used |
|---|---|---|
| **shadPS4**<br/>`shadPS4@be21649` | `src/core/loader/elf.h` - the container header and segment descriptors, as a reader sees them | shape of `self_header` and `self_segment`, and the flag bits |
| **fpPS4** | `sys/sys_types.pas` - the same structures in Pascal, written independently | the second reading. Confirmed the shape and supplied the header constant `00 01 01 12`, which shadPS4 splits into four unnamed fields |
| **OpenOrbis PS4 Toolchain** | `scripts/make_fself.py` - a *writer* | everything a reader never looks at: the extended info block, the control block, metadata, the footer, and the entry props layout. No reader could have supplied these |
| **prosperity**<br/>`prosperity@3475257` | `tools/pkg_extract/pkg_extract.py` | the package container, the nesting, and the crypto chain. Also a fourth independent confirmation of the container layout |
| **orbistoun** | `crates/orbistoun-elf/src/wrapper.rs`, orbistoun#D049 | **the current generation's magic**, observed from real material. Every other source above describes the previous generation only |

## Title metadata

| project | what it gave | how it was used |
|---|---|---|
| **pfd_sfo_tools** (flatz), vendored in `etaHEN` | `sfopatcher/src/sfo.{c,h}` - a `PARAM.SFO` **reader and writer** | the header, the index entries, the three-table layout, and the writing order |
| **ps5upload** | `engine/crates/ps5upload-{core,pkg}` - a reader and a writer in Rust, GPL-3 | the second, independent reading; the format codes; the `localizedParameters` structure of `param.json` |

Both are reference only, as everything above is: structure was read, recorded in
`data/sfo-format.tsv` with this provenance, and implemented from that record.

**And both were wrong about one rule.** They pad a written file to a sixteen-byte boundary;
eleven real files across three hardware generations do not. The table records the measured rule
and the refuted one side by side. See D019 - this is the first time a cited source has been
overturned by material rather than extended by it.

## Packages

| project | what it gave | how it was used |
|---|---|---|
| **LibProsperoPKG** (SvenGDK)<br/>`LibProsperoPKG@main` | `PFS/ProsperoPs5InnerMetadata.cs` - the inner superblock and inode layout as a **current-generation** library states them, including the index at `0xD8`; and the file list itself, which keeps a `Ps5` inner-image builder and flat path table separate from the previous generation's | confirmed `0xD8` is a field rather than padding, and is a second reading that also calls it unexplained. Settled a negative too: its `version = 2`, `mode = 0x18` inner format is **not** what the three real packages here use, so that variant was not chased |
| **LibOrbisPkg** (maxton)<br/>`LibOrbisPkg@6434772` | `PKG/Enums.cs` - the entry id names; `PKG/PkgBuilder.cs` - how each entry is produced and the assembly order; `Rif/LicenseDat.cs` - the licence structure; `PFS/PfsStructs.cs` - the filesystem superblock, inode and directory-entry layouts; `PFS/PFSBuilder.cs` - the block allocation and the signature ordering; `PFS/PfsProperties.cs` - that the inner filesystem is plain and the outer one holds a single file; `PFS/PFSCWriter.cs` - the container header; `Util/Crypto.cs` - the key derivations | named all six package entries this project could not, settled `PLAYGO_CHUNK_SHA`, named every superblock field, and supplied the whole filesystem-writing layout |

The filesystem writer (D049-D053) came from those four `PFS/` files and `Util/Crypto.cs`. Three
things in them were not conclusions this project would have reached on its own: that the
**inner** filesystem is unsigned and unencrypted, that `PFSC` **does not compress**, and the
order block signatures have to be computed in. Each was checked afterwards against real
packages, and the key derivation was checked in both directions - computed against recovered -
on every package to hand.

`Util/Keys.cs` also carries the debug RIF keyset and the key over a licence's secret, both read
from a local clone rather than transcribed - a summarising reader returned 519 hex characters
for a 512-character key, and four reconstructions were rejected by the signature check before
the file was read directly. (D047)

It is a **writer**, which is the third time that has been the thing that mattered: three readers
could not supply the container's metadata blocks and OpenOrbis' writer could, and the same
pattern repeats here. This repository was already relying on it indirectly - the public fake
keyset in `data/pkg-keys.toml` is published there and reached here by way of prosperity.

**The derivations came first, and the source agreed with them.** `0x1`, `0x100`, `0x80` and
`0x1002` were established from packages before this was read, and it names them `DIGESTS`,
`METAS`, `GENERAL_DIGESTS` and `PLAYGO_CHUNK_SHA`. A derivation that survives meeting its
source is worth more than either alone, and `selfish derive` still re-checks all four against
whatever packages a reader has.

## The lesson in that table

Three of the five are readers, and between them they could not supply the metadata blocks -
because a reader walks a container to find the executable and never looks at them. The writer
did. And the current generation's magic came from none of the four, but from a sibling project
that had observed it directly.

No single source was sufficient, and the one that mattered most for the actual target was the
one nobody thought to check.

**PS4-Store** (LightningMods) - `Store/pkg.gp4` and three `sce_sys/param.sfo` files. Homebrew
that actually ships, used here as an oracle rather than a source: it confirmed the fake passcode
independently (written out in the project file), and its `param.sfo` files are what made the
generation split in the metadata table visible. See D061.

## How a source is cited here

**By project and commit**, so that "where did this come from" has an answer somebody else can
check. A few entries above read `@vendored` instead: those were read from a copy taken at an
unrecorded point, and saying so is the honest form - it marks the citation as weaker rather
than dressing it as a commit nobody can resolve.

Two entries carry no commit at all yet, and that is a gap rather than a decision. Anything
added from here needs one.
