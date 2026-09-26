# Glossary

The vendor formats' vocabulary. Standard ELF terms, and words that mean different things in
different repositories, are in
[the collection's glossary](https://github.com/project-oops/OOPS/blob/main/docs/GLOSSARY.md).
The tables in `data/` are the source of truth; each entry names where its format lives.

## The generation split

**Generation** - which machine a file is for: Orbis-generation or Prospero-generation. The two
share one container format and differ in its four-byte magic. It is a type with no default
(D002). *`selfish-abi`, `data/self-format.tsv`.*

**The container magic** - `4F 15 3D 1D` or `54 14 F5 EE`. A current-generation app eboot
carries the first and a title's bundled modules the second (`obscene#D298`); what makes a title
native is `param.json` and native registration, not the magic.

## Executables

**NID** - the import hash. A vendor module names an imported symbol by a hash of its name,
encoded as eleven characters: the first 8 bytes of `SHA-1(name || suffix)`, read little-endian.
*`selfish-nid`; the mined name corpus is obSCEne's.*

**SELF** - the signed-executable container an ELF is wrapped in. A retail one is signed with
keys only the vendor holds.

**fSELF** - a fake SELF: it declares itself fake in the field the format provides, with every
digest and the signature area zero. *`selfish-container`.*

**eboot, `eboot.bin`** - a title's main program, in container form; what the system loader
starts.

**Vendor segment** - `PT_SCE_DYNLIBDATA`, which carries the dynamic tables in the platform's
layout. `DT_SCE_*` tags point into it. *`selfish-elf::dynamic`, `selfish-elf::dynlib`.*

**`PT_SCE_PROCPARAM`, `PT_SCE_MODULE_PARAM`** - a block a program or module carries that the
loader reads before running any of its code, including the SDK version it was built against.

**`module_start`, `module_stop`** - a module's load and unload entry points, exported by NID.

## Titles

**`PARAM.SFO`** - a binary key-value table describing a title; every package carries one.

**`param.json`** - the current generation's title description, written into a title directory.
*Both in `selfish-title`.*

**`applicationCategoryType`** - the `param.json` field (`CATEGORY` in `PARAM.SFO`) that decides
a title's memory budget and whether it owns the display. Independent of the container's `paid`
privilege tier. The values are in the collection's
[four-axes table](https://github.com/project-oops/OOPS/blob/main/docs/CONVENTIONS.md#the-four-axes-of-a-build-and-a-run)
and `selfish_title::category`.

**Title directory** - a title as files: an eboot beside `sce_sys/`, which the install call or
an auto-mounter registers.

**`CONTENT_ID`, `TITLE_ID`** - a title's identity. The title id is the field of the content id
between the first `-` and the `_`.

## Packages

**Package (`.pkg`)** - four nested formats:

```text
.pkg  ->  header and entry table
      ->  filesystem image at 0x80000, encrypted
      ->  a PFSC image inside it
      ->  the inner filesystem: the title's files
```

**PFS** - the filesystem inside a package, read through three layers:

```text
raw bytes
  -> XTS    sector-by-sector decryption, 4 KiB sectors
  -> PFSC   blocks addressed through a map, zlib-compressed or stored
  -> PFS    a superblock, inodes and directories
```

*`selfish-pfs`.*

**Keystone** - `sce_sys/keystone`, 96 bytes derived from the passcode. *`selfish-pkg::keystone`.*

**playgo** - the chunk and scenario descriptors a package carries, saying how a title may run
while it installs.

**Licence, RIF** - the entry that says a package may be installed. The ones built here are
signed with the published debug keyset and declare themselves debug licences (D047).

**Fake keysets** - the published package keysets in `data/pkg-keys.toml`. Nothing here works on
retail material; real files confirm or refute a structure and are never committed.
