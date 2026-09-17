# Retail Packages (PKG)

Authoring encrypted PlayStation File System (PFS) packages for retail installation.

PlayStation software packages (`.pkg`) combine an outer header table, playgo chunk manifests, encryption keys, and an inner encrypted PFS filesystem image.

---

## Recipe: Author an Installable Package

Like every pipeline format, `--format pkg` takes the compiled **ELF** as `--input`; it lays out the title internally and assembles the package around it.

```bash
selfish --input build/gl-cube.elf \
        --target prospero \
        --format pkg \
        --title-id GLCB00001 \
        --output build/GLCB00001.pkg
```

To assemble a package around an existing tree of files instead of a single ELF, use the `selfish pack --dir <root>` command.

### What SELFish Synthesizes:
1. **PFS Filesystem Image**: Packs all application files into an encrypted 64KB block filesystem with inode tables and Merkle tree hashes.
2. **Playgo Manifest**: Builds playgo chunk tables that determine initial download chunks for background installation.
3. **Outer Package Header**: Encodes content ID, volume sizes, digest tables, and RSA-signed licences.
4. **Key Encryption**: Wraps the PFS encryption key using standard retail fake-passcode algorithms.

