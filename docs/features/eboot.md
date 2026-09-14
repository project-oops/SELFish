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
3. **Digest Blocks**: Computes SHA-256 block digests across all loadable segments.
4. **Signature Metadata**: Stamps fake RSA signatures accepted by jailbroken hardware loaders and Orbistoun.

---

## Command Flags

- `--privilege <app|system|root>`: Controls security privilege tier (defaults to `app`).
- `--sdk <version>`: Sets expected platform SDK version stamp (e.g. `0x12400000` for FW 12.40).

