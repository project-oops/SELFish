# D039 - LibOrbisPkg named all six, and the derivations were right


The source was fetched and read: `LibOrbisPkg/PKG/Enums.cs` names every entry id,
`PKG/PkgBuilder.cs` says how each is produced, `Rif/LicenseDat.cs` gives the licence structure.
It is an open-source PS4 **packaging tool** - a writer, which is the third time a writer has
been the thing that mattered.

| entry | name | this project had |
|---|---|---|
| `0x1` | `DIGESTS` | derived: one digest per entry, table order, self-slot zero |
| `0x80` | `GENERAL_DIGESTS` | derived: digests of named things |
| `0x100` | `METAS` | derived: the entry table again |
| `0x1002` | `PLAYGO_CHUNK_SHA` | unknown |
| `0x400` | `LICENSE_DAT` | unknown |
| `0x401` | `LICENSE_INFO` | unknown |

**Four derivations, four agreements.** They were established from packages before the source
was read, and the source called them the same things. That is worth recording in that order:
a derivation which survives meeting its source is stronger evidence than either alone, and
`selfish derive` still re-checks all four against whatever packages a reader has.

