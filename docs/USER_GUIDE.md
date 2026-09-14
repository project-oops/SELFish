# SELFish User Guide & Packaging Cookbook

Welcome to the **SELFish** user guide.

This guide provides clean, copy-pasteable recipes for **homebrew developers and packagers** who want to transform raw compiled ELF binaries into bootable signed containers (`eboot.bin`), structured retail title directories, and installable packages (`.pkg`).

If you are an LLM agent, format researcher, or compiler engineer seeking internal crypto specifications, segment header tables, or decision records, see the **[Technical Reference](README.md)** and **[DECISIONS.md](DECISIONS.md)** instead.

---

## Table of Contents

1. [Quickstart & Syntax](#1-quickstart--syntax)
2. [Packaging Recipes](#2-packaging-recipes)
   - [Recipe 1: Wrap an ELF into a Signed Executable (`eboot.bin`)](#recipe-1-wrap-an-elf-into-a-signed-executable-ebootbin)
   - [Recipe 2: Wrap an ELF into a Shared Library (`.prx`)](#recipe-2-wrap-an-elf-into-a-shared-library-prx)
   - [Recipe 3: Generate a Full Retail Title Directory](#recipe-3-generate-a-full-retail-title-directory)
   - [Recipe 4: Author an Installable Package (`.pkg`)](#recipe-4-author-an-installable-package-pkg)
3. [Inspecting & Auditing Binaries](#3-inspecting--auditing-binaries)
4. [Common Errors & Troubleshooting](#4-common-errors--troubleshooting)

---

## 1. Quickstart & Syntax

The `selfish` command uses a unified four-axis structure for file generation:

```bash
selfish --input <file> --target <generation> --format <format> --output <path>
```

### Supported Parameters:
- `--target`: Platform generation: `prospero` (default, PS5), `orbis` (PS4), `neo` (PS4 Pro), or `trinity` (PS5 Pro).
- `--format`: Desired output format:
  - `eboot`: Main signed application container (`eboot.bin`).
  - `prx`: Dynamic shared module (`.prx`).
  - `title`: Complete game directory layout (`eboot.bin`, `sce_sys/param.json`, `keystone`, `icon0.png`, etc.).
  - `pkg`: Installable encrypted PFS container (`.pkg`).
  - `elf`: Stripped/relocated ELF output.

---

## 2. Packaging Recipes

### Recipe 1: Wrap an ELF into a Signed Executable (`eboot.bin`)

Wrap a standard freestanding ELF binary into a fake-signed executable container runnable on jailbroken hardware or in Orbistoun:

```bash
selfish --input build/my_app.elf --target prospero --format eboot --output build/eboot.bin
```

---

### Recipe 2: Wrap an ELF into a Shared Library (`.prx`)

Convert a dynamic library ELF (e.g. custom `libc.so`) into a platform-conforming PRX module:

```bash
selfish --input build/libc.so --target prospero --format prx --output build/sce_module/libc.prx
```

---

### Recipe 3: Generate a Full Retail Title Directory

Jailbroken consoles running retail environments (`BIG_APP`) require a specific directory hierarchy and metadata files. SELFish generates all required metadata automatically:

```bash
selfish --input build/gl-cube.elf \
        --target prospero \
        --format title \
        --title-id GLCB00001 \
        --category big-app \
        --name "GL-Cube 3D Demo" \
        --output build/title/GLCB00001
```

#### What SELFish Automatically Generates:
```
build/title/GLCB00001/
├── eboot.bin                <- Signed executable container
├── sce_module/
│   └── libc.prx             <- Companion runtime module
└── sce_sys/
    ├── param.json           <- Conforming metadata (titleId, category: 0, appVersion)
    ├── icon0.png            <- Conforming 512x512 RGB application icon
    ├── keystone             <- Fake-signed keystone paired with generated passcode
    ├── nptitle.dat          <- Network title identifier token
    └── pfs-version.dat      <- PFS filesystem version stamp
```

You can now stage this entire directory directly with `pros restore`!

---

### Recipe 4: Author an Installable Package (`.pkg`)

Package your homebrew into an installable `.pkg` archive:

```bash
selfish --input build/title/GLCB00001 \
        --target prospero \
        --format pkg \
        --output build/GLCB00001.pkg
```

---

## 3. Inspecting & Auditing Binaries

SELFish includes built-in diagnostic tools to inspect unknown binaries, check import hashes, and audit container signatures:

### Check File Headers & Container Type
```bash
selfish container build/eboot.bin
# Output:
# Target: Prospero
# Container Magic: 54 14 F5 EE (SELF)
# Segments: 3 loaded, 1 digest
```

### Inspect NID Imports & Symbol Hashes
```bash
selfish imports build/my_app.elf
# Output:
# libkernel:
#   0x2E68BD659B8B690A (sceKernelUsleep)
#   0x3E18C7E7608670A1 (sceKernelAllocateDirectMemory)
```

### Audit Relocations
```bash
selfish reloc build/my_app.elf
# Output:
# Total relocations: 142
# Conformance: 100% standard x86-64 R_X86_64_RELATIVE / R_X86_64_64
```

---

## 4. Common Errors & Troubleshooting

### Error: `unsupported relocation type R_X86_64_...`
- **Cause**: The compiler emitted a relocation type not permitted in freestanding console binaries (e.g. thread-local storage relocations or GOT offsets).
- **Fix**: Recompile with `-fno-pie -fno-pic -mcmodel=large` or use the standard `oops-sdk/common/app.mk` flags.

### Error: `missing entry point in ELF`
- **Cause**: The ELF does not export an entry point or `_start` symbol.
- **Fix**: Ensure your entry point is marked `extern "C" void _start(void)` and your linker script places it at the image base.

### Error: `target rejected container (bad magic)`
- **Cause**: The executable was compiled targeting the wrong hardware generation (e.g. Orbis instead of Prospero).
- **Fix**: Specify `--target prospero` explicitly during packaging.

