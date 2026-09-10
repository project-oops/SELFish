# D103 - The AGC shader container is admissible as a format, but only the part a citable source derives - not the vendor-binary part, and not because hardware accepts it

**Status: decided. 2026-09-10.**


**A shader container is a format and belongs here (obSCEne REQ-20260910T1945Z-7a3b). Most of its
structure is derivable from an open-source emulator and is admissible. The fields that make it a
*working* container are, in the request's own draft, worked out by disassembling `libSceAgc.sprx` -
which principle 1 forbids as a source, and which a passing hardware sweep does not cure. So SELFish
does not yet ship the full container, and the draft cannot be committed as written.**

## The request, and why it is in remit

obSCEne asks SELFish to build the 304-byte (`0x130`) `sceAgcCreateShader` container header from raw
RDNA2 bytecode, rather than hard-code the bytes in a consumer. That is the admission test passed:
a container header is knowledge about a file format, it would make sense if the three consumers did
not exist, and hard-coding it in obSCEne is the duplication this repository exists to remove. The
question is never whether it belongs - it is where the structure may come from.

## Two sources, and only one of them counts

The request cites two things, and they are different in kind:

- **obSCEne's hardware sweep** `20260910-203426-eboot` (`166-agc/compute-dispatch`: `rc-create=0x0`,
  `fence-hit=0x1`). This proves the container *works* on FW 12.40. It is an **oracle** - principle 2,
  "confirm or refute, never derive". It cannot be the source of a field's structure.
- **The draft** `selfish/docs/worklog/069-...` (untracked) and obSCEne's `169-...`, which state the
  layout came from *"deep disassembly of `libSceAgcDriver.sprx`, `AgcCompositor.elf`, and
  `libSceAgc.sprx`"*. That is **reading a vendor binary**, which principle 1 forbids in as many
  words: *"no format worked out by reading a vendor binary ... reimplementation-from-a-binary
  converges on the original, and that convergence is visible to anyone who looks."*

A container that works and whose structure was disassembled out of the vendor's own library is the
exact case principle 1 is written for. Hardware saying yes does not change where the bytes came
from.

## There is a citable source, and it covers most of the header

**craziiEmu** - an open-source PS5 emulator - implements `sceAgcCreateShader` in
`src/CraziiEmu.Libs/Agc/AgcExports.cs` (`craziiEmu@8a13647`). Read as a reader of the header, it
establishes, citably:

| field | offset | craziiEmu |
|---|---|---|
| magic `0x34333231` (`"1234"`) | `0x00` | `ShaderFileHeader`, validated |
| version `0x18` | `0x04` | `ShaderVersion`, validated |
| user-data pointer | `0x08` | `ShaderUserDataOffset` |
| code pointer | `0x10` | `ShaderCodeOffset`, written with the bytecode address |
| context registers | `0x18` | `ShaderCxRegistersOffset` |
| sh registers | `0x20` | `ShaderShRegistersOffset` |
| specials | `0x28` | `ShaderSpecialsOffset` |
| input semantics | `0x30` | `ShaderInputSemanticsOffset` |
| output semantics | `0x38` | `ShaderOutputSemanticsOffset` |
| shader type (`0` = compute) | `0x5A` | `ShaderTypeOffset` |
| sh register count | `0x5C` | `ShaderNumShRegistersOffset` |

That is the magic, the version, the whole pointer table, and the two stage bytes - and the register
count at `0x5C` is exactly the request's "register count at `+0x5c`". This much is admissible: it
comes from a named open-source implementation and the hardware sweep confirms it.

## What is left is only in the vendor disassembly, and stays out

craziiEmu does not model these, and the draft's only source for them is `libSceAgc.sprx`:

- **target ISA `0x0e`** - no ISA field appears in craziiEmu.
- **the barefoot trailer hash at `+0x44`** - a 64-bit hash of the payload; craziiEmu reads no such
  field.
- **the total size `0x130` and stage size `0xd8`** - craziiEmu reaches `0x5C` and stops; the total
  is the disassembly's.
- **the 256-byte payload alignment** - not in craziiEmu.

These are the fields that make the difference between a header a console parses and one it rejects,
and they have no citable derivation - only a vendor binary, plus a sweep that confirms the whole
thing works. Principle 5 forbids filling them from the binary and calling the result a container:
a `0x130` buffer that looks complete but carries disassembled offsets is worse than none, because
the next reader cannot tell which bytes are established and which were lifted.

## The ruling

1. The format is **in remit** and will live here when it can be sourced, not in a consumer.
2. The craziiEmu-derived subset (the table above) is **admissible** - cite `craziiEmu@8a13647`,
   confirm against obSCEne's sweep as the oracle.
3. The vendor-disassembly fields are **not admissible** on that provenance, and a passing sweep does
   not promote them - it confirms, it does not derive.
4. **SELFish does not ship the full container yet**, because the working-critical fields are exactly
   the ones with no citable source. A partial container is not shipped either: principle 5.
5. The untracked `069` draft is **not committed** - its stated provenance is the disassembly, which
   is the one thing that cannot enter this repository.

## What would unblock it

A citable source for the four fields above - craziiEmu growing shader-creation coverage past
`0x5C`, another open-source AGC implementation, or a published RDNA2 / AGC reference for the ISA
constant and the trailer. With any of those, the container becomes fully derivable and the sweep
confirms it. Until then this is where the work honestly stops, and saying so is the point of the
repository, not a failure of it.
