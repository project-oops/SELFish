# Glossary: the formats

What the words mean, for somebody who has not worked with these formats before.

This is the **vendor half** of the collection's vocabulary. The standard-ELF half - what `DT_`
and `PT_` mean at all, what `.bss` is, what a segment is against a section - is
[the collection's glossary](https://github.com/project-oops/OOPS/blob/main/docs/GLOSSARY.md),
and it is worth reading first: everything below is an extension bolted onto that, using the
same mechanisms with private numbers.

Every entry names where it is established. The `data/*.tsv` tables are the source of truth and
carry provenance per row; this page is prose over them, not a second copy of them.

## The generation split

**Generation** - which console a file is for. Two generations share one container format and
differ in four bytes. This is a type here, not a runtime parameter: a builder that does not
say which one it is targeting does not compile, because the one time it was a parameter with a
default, the default was wrong and the file was rejected by the machine it was built for.

*`selfish-abi`, and `data/self-format.tsv`.*

**The four bytes** are the container magic, and the labelling is subtler than it looks.
`4F 15 3D 1D` is what a real current-generation *app eboot* carries; `54 14 F5 EE` is what a
title's *bundled modules* carry. Both are current. Calling the second one "the current
generation's magic" was a hypothesis the table itself flagged, and hardware refuted it - see
obSCEne's D293. What makes a title native is `param.json` and native registration, not the
eboot's magic.

## Executables

**NID** - the import hash. A vendor module imports by a hash of the symbol name rather than by
the name, so a module's import list is a list of numbers until you can reverse them. This is
the difference between a resolvable module and an opaque blob.

```text
NID = first 8 bytes of SHA-1(name || suffix), read little-endian
```

*`selfish-nid`. The suffix is a fixed constant; the mined corpus of known name-to-NID pairs is
obSCEne's, not this repository's - it is a measurement product, not a format fact.*

**SELF** - the signed-executable container. What the platform wraps an ELF in. A retail one is
signed with keys nobody outside the vendor has.

**fSELF** - a *fake* SELF. Declares itself fake in the field the format provides for exactly
that, with every digest and the whole signature area left zero. Nothing here forges a vendor
signature, and nothing could.

*`selfish-container`, which reads and writes both directions on purpose: the half that writes
is where the errors are, and keeping them together makes a round trip a test.*

**eboot / `eboot.bin`** - the main program of a title, in container form. The thing the system
loader is pointed at.

**Vendor segment** - a `PT_SCE_*` segment carrying the dynamic tables in the layout the
platform expects, rather than the standard arrangement. `DT_SCE_*` tags live in it.

**`PT_SCE_PROCPARAM` / `PT_SCE_MODULE_PARAM`** - a small block a program or module carries
describing itself to the loader before any of its code runs, including the SDK version it was
built against. `libkernel` reads it first, so a field it expects and does not find faults
inside a platform library on a stack frame naming nothing of yours.

**`module_start` / `module_stop`** - the conventional names for a module's "you have been
loaded" and "you are being unloaded" entry points. Exported by NID like anything else, so
asking whether a module has them means asking whether those three hashes appear in its export
table.

## Titles

**`PARAM.SFO`** - a binary key-value table describing a title. Appears inside every package.

**`param.json`** - what the current generation writes into a title directory. Same job, one per
generation, both read and written here.

*`selfish-title`. It depends on nothing else in this repository: it holds what a title says
about *itself*, which is a different kind of fact from a container layout.*

**Title directory** - a title laid out as directories and files rather than packed into a
package: an eboot beside `sce_sys/`. What an auto-mounter can register.

**`CONTENT_ID` / `TITLE_ID`** - the identity of a title. The title id is a field *inside* the
content id, so it is derived rather than written twice.

## Packages

**Package (`.pkg`)** - not an archive. Four nested formats:

```text
.pkg  ->  header + entry table
      ->  filesystem image at 0x700000     <- encrypted
      ->  a compressed image inside that
      ->  the real filesystem: files, each executable a container
```

**PFS** - the filesystem inside a package. Three layers, each wrapping the one below:

```text
raw bytes
  -> XTS    sector-by-sector decryption, 4KiB sectors
  -> PFSC   zlib-compressed blocks addressed through a map
  -> PFS    a superblock, inodes, and directories
```

*`selfish-pfs`. Note that PFSC does not have to actually compress: the block map is a list of
absolute offsets, and blocks laid end to end at a fixed stride are a valid map.*

**Keystone** - a fixed-size blob a package carries. Derived from the passcode rather than
supplied.

**playgo** - the chunk and scenario descriptor a package carries, describing how a title may be
played while still installing. Written in full rather than as a header, because a console reads
the counts and then looks for the records they promise.

**Licence / RIF** - the file that says a package may be installed. Ours are signed under the
**debug** keyset, a published keypair whose entire purpose is the fake licences a non-retail
package holds, and the licence says it is a debug licence in its own type field. That is the
opposite of a forgery: the line is never claim to be the vendor, not never compute a signature.

## Keys, and what this repository will not do

**The public fake-package keysets only.** Nothing here works on retail material and nothing
here should be made to. Real files are used as an **oracle** - to confirm or refute a structure
taken from a citable source - never to derive one, and the material itself is never committed.

*[CLAUDE.md](../CLAUDE.md) principles 1, 2 and 6.*

## Where the rest is

- [the collection's glossary](https://github.com/project-oops/OOPS/blob/main/docs/GLOSSARY.md) - standard ELF, and words that mean different things in different repositories
- [obSCEne](https://github.com/project-oops/obSCEne/blob/main/docs/GLOSSARY.md) - checks, the census, execution contexts
- [orbistoun](https://github.com/project-oops/Orbistoun/blob/main/docs/GLOSSARY.md) - guest execution, thunks, HLE
- [Prosperous](https://github.com/project-oops/Prosperous/blob/main/docs/GLOSSARY.md) - targets, chains, scan roots
