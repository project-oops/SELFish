# SELFish User Guide

Cross-cutting reference for how SELFish behaves under the hood — command syntax, portable mode, format axes, diagnostic checks, and common error recovery. Per-format pages live alongside; this page covers tool-wide usage.

---

## The Four-Axis Command Line

Packaging in SELFish is always one invocation along four clear axes:

```bash
selfish --input <file> --target <generation> --format <format> --output <path>
```

### 1. `--target <generation>`
Sets the platform hardware generation. Never defaulted, because a container stamped with the wrong generation's magic bytes will be rejected by target hardware:
- `prospero` (PS5)
- `orbis` (PS4)
- `neo` (PS4 Pro)
- `trinity` (PS5 Pro)

### 2. `--format <format>`
- `eboot`: Signed standalone application executable (`eboot.bin`).
- `prx`: Platform dynamic shared library (`.prx`).
- `title`: Complete retail title directory (`eboot.bin`, `sce_sys/param.json`, `keystone`, `icon0.png`, `nptitle.dat`, `pfs-version.dat`).
- `pkg`: Encrypted PFS package for installation.
- `elf`: Stripped and relocated standard ELF.

---

## Paths and Portable Mode

SELFish reads and writes only the paths specified on the command line:
- `--output` specifies the exact file or directory to produce. Nothing is ever written beside it.
- Portable mode is active by default because `selfish` has no global registry and writes no host files outside the `--output` destination.
- When invoked in pipelines, temporary scratch files are placed under the system temp root (never beside `--output`) and removed before the command returns.

---

## Diagnostics & Inspection

SELFish includes built-in diagnostic tools to inspect unknown binaries:

```bash
# Inspect container header and segments
selfish container build/eboot.bin

# Inspect NID import hashes
selfish imports build/my_app.elf

# Check relocations
selfish reloc build/my_app.elf
```

---

## Error Recovery

### `unsupported relocation type R_X86_64_...`
- **Cause**: The compiler produced position-dependent or TLS relocations.
- **Fix**: Recompile with `-fno-pie -fno-pic -mcmodel=large` (included in `oops-sdk/common/app.mk`).

### `missing entry point in ELF`
- **Cause**: Linker did not find `_start`.
- **Fix**: Ensure entry point is declared as `extern "C" void _start(void)` in your root translation unit.

