# SELFish Features & Packaging Recipes

Feature documentation and packaging recipes for SELFish. Each page covers command-line options, format specifications, and verification steps.

| Feature / Format | Command Axis | Output Artefact | Documentation Page |
| :--- | :--- | :--- | :--- |
| **User Guide** | `selfish --help` | System-wide paths, portable mode, error recovery | [user-guide.md](user-guide.md) |
| **Signed Executable** | `--format eboot` | `eboot.bin` (Signed container) | [eboot.md](eboot.md) |
| **Title Directory** | `--format title` | Complete retail title directory (`param.json`, `keystone`, `icon0.png`) | [title.md](title.md) |
| **Retail Package** | `--format pkg` | `.pkg` (Encrypted PFS container) | [pkg.md](pkg.md) |
| **Reading & Audit** | `selfish container`, `elf`, `imports` | Diagnostic header dumps | [reading.md](reading.md) |
| **Pipeline Writing** | `selfish --input ...` | Four-axis compilation pipeline | [writing.md](writing.md) |
| **Crate Spine** | Rust dependencies | `selfish-abi`, `selfish-elf`, `selfish-pfs` | [library.md](library.md) |

