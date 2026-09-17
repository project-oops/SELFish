# Title Directories

Authoring full retail title directories with conforming metadata, icons, and keystones.

When deploying an application to run as a full-screen retail game (`BIG_APP`), the console's application manager requires a structured directory layout containing specific metadata files.

---

## Recipe: Generate a Retail Title Directory

```bash
selfish --input build/gl-cube.elf \
        --target prospero \
        --format title \
        --title-id GLCB00001 \
        --category big-app \
        --title "GL-Cube 3D Demo" \
        --output build/title
```

`--output` is the parent directory; the title is laid out at `<output>/<TITLE_ID>/`, so `build/title` yields `build/title/GLCB00001/`.

### Generated Layout

```text
build/title/GLCB00001/
├── eboot.bin                <- Signed executable container
└── sce_sys/
    ├── param.json           <- Conforming metadata (titleId, category: 0, appVersion)
    ├── icon0.png            <- Conforming 512x512 RGB application icon
    ├── keystone             <- Fake-signed keystone paired with generated passcode
    ├── nptitle.dat          <- Network title identifier token
    └── pfs-version.dat      <- PFS filesystem version stamp
```

---

## Key Metadata Fields Synthesized

1. **`param.json`**:
   - `titleId`: Matches `--title-id` (e.g. `GLCB00001`).
   - `category`: `0` for `BIG_APP` (retail full-screen foreground application).
   - `localizedParameters`: Contains application name shown on console dashboard.
2. **`keystone`**: Cryptographic passcode binder used by the console to pair game save files.
3. **`icon0.png`**: Standard 512×512 PNG icon displayed on the home-screen dashboard.

