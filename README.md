<p align="center">
  <img src="assets/logo.png" alt="SELFish" width="200">
</p>

# SELFish

**Clean-Room File Format Compiler, Container Signer, and Packaging Toolchain.**

SELFish is a clean-room Rust toolchain for reading, writing, and authoring 8th and 9th generation console file formats (Orbis and Prospero). It transforms standard ELF binaries produced by compilers into signed executables (`eboot.bin`), structured title directories with conforming metadata (`param.json`, `keystone`, `nptitle.dat`), and installable packages (`.pkg`).

Site: **[project-oops.github.io/SELFish](https://project-oops.github.io/SELFish/)**

| 📖 **[User Guide & Packaging Cookbook](docs/USER_GUIDE.md)** | ⚙️ **[Technical Reference & Format Specs](docs/README.md)** |
| :--- | :--- |
| *Copy-paste recipes for eboot, title directory, PKG, and ELF audit.* | *Crypto headers, container layout, NID hashes, and decision records.* |

---

## Role in THE LOOP

Within the [OOPS ecosystem](../docs/THE_LOOP.md), SELFish is the **Packaging and Binary Layout Engine**:

```
Compiled ELF (from oops-apps or obSCEne)
                 │
                 ▼
┌────────────────────────────────────────┐
│ SELFish Toolchain                      │
│ - Wraps ELF in SELF container header   │
│ - Synthesizes param.json & keystone    │
│ - Sets BIG_APP category & permissions  │
│ - Creates conforming title layout      │
└────────────────┬───────────────────────┘
                 │
       [Conforming Container]
                 │
                 ├───────────────────────────────┐
                 ▼                               ▼
     Prosperous (pros restore)        Orbistoun (orbistoun run)
     Deployed to Physical PS5         Executed in Native Emulator
```

1. **Eliminating Leaked SDK Tools**: Traditional homebrew relies on leaked vendor utilities (`orbis-pub-cmd`) to create packages. SELFish replaces them with 100% clean-room, mathematically verified Rust libraries.
2. **First-Party Dogfooding**: Every title in [oops-apps](../oops-apps/) and every test leg in [obSCEne](../obscene/) is packaged via `selfish`.
3. **Format Integrity**: Because both the real console and the [Orbistoun](../orbistoun/) emulator read the containers authored by SELFish, format inconsistencies are caught at build time.

---

## Developer Quickstart

### 1. Build and Verify
```bash
./bin/selfish check    # compiles crates and runs verification tests
```
The compiled binary lives at `target/release/selfish.exe` (Windows) or `target/release/selfish` (Linux/macOS).

### 2. Common Packaging Workflows

#### Author a Full Title Directory
```bash
selfish --input gl-cube.elf \
        --target prospero \
        --format title \
        --title-id GLCB00001 \
        --title "GL Cube" \
        --category big-app \
        --output build/title
```
Automatically generates:
- `build/title/GLCB00001/eboot.bin` (Signed executable)
- `build/title/GLCB00001/sce_sys/param.json` (Title metadata)
- `build/title/GLCB00001/sce_sys/keystone` (Fake-signed passcode anchor)
- `build/title/GLCB00001/sce_sys/icon0.png` (Default application icon)
- `build/title/GLCB00001/sce_sys/nptitle.dat` and `pfs-version.dat`

#### Create a Signed `eboot.bin`
```bash
selfish --input payload.elf --target prospero --format eboot --output eboot.bin
```

#### Inspect Binary Files
```bash
selfish elf <file>         # inspect ELF headers, segments, and dynamic tags
selfish imports <file>     # list imported library symbols resolved by NID hash
selfish title <file>       # read title metadata from param.json or PARAM.SFO
selfish pkg <file>         # inspect contents of an installable package
```

---

## Architecture & Crates

A modular crate spine where each layer depends only on those below it:

| Crate | Purpose |
|---|---|
| **`selfish-abi`** | Target generation constants (`orbis`, `neo`, `prospero`, `trinity`). |
| **`selfish-nid`** | SHA-1 symbol NID import-hash computation and encoding (name cracking lives in obSCEne, not here). |
| **`selfish-elf`** | ELF64 header validation, vendor dynamic table, and relocation tables. |
| **`selfish-container`** | SELF (Signed ELF) container reading and writing. |
| **`selfish-title`** | Title metadata generation (`param.json` and `PARAM.SFO`). |
| **`selfish-pfs`** | PlayGo / PFS package filesystem structure. |
| **`selfish-pkg`** | Cryptographic authoring and packaging of `.pkg` archives. |
| **`selfish-shader`** | AGC shader container (`sceAgcCreateShader`) authoring, off to the side. |
| **`selfish-cli`** | The unified `selfish` command-line executable. |

---

## Cross-Project Links

- **[Master OOPS Front Door](../README.md)** — Collection overview and building instructions.
- **[The OOPS Loop](../docs/THE_LOOP.md)** — Master ecosystem loop specification.
- **[oops-apps](../oops-apps/)** — Applications packaged using SELFish.
- **[obSCEne](../obscene/)** — Conformance probe legs packaged using SELFish.
- **[Prosperous](../prosperous/)** — Deployment tool that transfers SELFish title trees.
- **[Orbistoun](../orbistoun/)** — Clean-room emulator executing SELFish containers.
