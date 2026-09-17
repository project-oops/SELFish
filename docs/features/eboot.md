# Signed Executables (eboot)

Wrapping raw ELF executables into signed SELF/fSELF containers runnable on target hardware.

On PlayStation platforms, executables do not run as raw ELFs; they must be enclosed in signed executable containers carrying segment digests, entry points, and privilege tags.

---

## Recipe: Wrap an ELF into an `eboot.bin`

```bash
selfish --input build/my_app.elf \
        --target prospero \
        --format eboot \
        --output build/eboot.bin
```

### What SELFish Does:
1. **ELF Validation**: Inspects program headers (`PT_LOAD`) and verifies all segments are 4KB-aligned.
2. **Container Header**: Writes standard 32-byte header with generation magic (`54 14 F5 EE` for Prospero).
3. **Digest Area**: Reserves one digest entry per segment. The container declares itself **fake** in the field the format provides for that, so the digest area is left **zero** — nothing is hashed.
4. **Signature Area**: Left **zero** as well. No vendor signature is forged and none could be; the fake marker is what a homebrew loader (and Orbistoun) accepts.

---

## Command Flags

- `--privilege <app|sysmodule|system|root>`: Controls the privilege tier the container declares (defaults to `app`; `sysmodule` currently produces bytes identical to `app`).
- `--sdk <version>`: Sets expected platform SDK version stamp (e.g. `0x12400000` for FW 12.40).

