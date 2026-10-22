# PAA Viewer & Converter

Cross-platform texture viewer, converter, and power-of-two (POT) resizer for Bohemia Interactive PAA files (Arma 3, DayZ, Real Virtuality engine), written in 100% pure Rust.

## Features

- **GPU-Accelerated Native GUI**:
  - Ultra-fast cross-platform viewer with zero external desktop dependencies. Perfect for standalone use on Linux and Windows.
  - RGBA channel inspection: view Full RGBA, RGB only, individual Red, Green, Blue channels, or Alpha as grayscale.
  - Real-time pixel inspector: hover cursor over any pixel to view (X, Y) coordinates, RGBA values, hex code `#RRGGBBAA`, and color swatch.
  - Drag & Drop: drag any `.paa`, `.png`, `.jpg`, `.tga`, `.bmp`, or `.webp` into the window to load immediately.
  - Single-instance activation: opening another file switches the running window and brings it forward.

- **System Context Menu & File Association Integration**:
  - **Linux Integration**:
    - FreeDesktop `.desktop` file registration
    - MIME type definition for `image/x-paa` (`*.paa`, `*.pac`)
    - Dolphin / KDE service menu actions (`~/.local/share/kio/servicemenus/paa-actions.desktop`)
    - Context menu actions: "Open with PAA Viewer", "Convert to PNG", "Convert to PAA (DXT5 POT)"
    - In-GUI toggle button or CLI: `paa-viewer context-menu install` / `uninstall`
  - **Windows Explorer Integration**:
    - Right-click context menu options for `.paa` files ("Open with PAA Viewer", "Convert to PNG")
    - Right-click context menu options for `.png`, `.jpg`, `.tga` files ("Convert to PAA (DXT5 POT)")
    - Instant registration via CLI, GUI button, or pre-generated `paa-viewer-context-menu.reg`

- **Full PAA Format Codec**:
  - Formats supported: DXT1, DXT2, DXT3, DXT4, DXT5, ARGB8888, ARGB1555, ARGB4444, GRAYA
  - LZO1X-1 / LZSS compression and safe decompression for DXT mipmaps
  - Bohemia LZ77 compression and decompression for ARGB mipmaps
  - Complete `GGAT` tagged sections parser and writer:
    - `AVGC` / `CGVA`: Average color RGBA
    - `MAXC` / `CXAM`: Max color RGBA
    - `FLAG` / `GALF`: Alpha transparency flag
    - `OFFS` / `SFFO`: Mipmap offset table (16 entries)
    - Custom tags (e.g. `SWAV`)
  - Automatic mipmap generation down to 4x4 (for DXT) or 1x1

- **Power of Two (POT) Resizer**:
  - Scaling factors: halve resolution (0.5x), double (2x), quadruple (4x), or custom factor
  - Power of Two alignment:
    - `pot-nearest`: round each dimension to the closest power of 2 ($2^n$, e.g. 500x300 -> 512x256)
    - `pot-next`: round up each dimension to the next power of 2 ($2^n$, e.g. 500x300 -> 512x512)
    - `pot-prev`: round down each dimension to the previous power of 2 ($2^n$, e.g. 500x300 -> 256x256)
    - `stretch:WIDTHxHEIGHT`: stretch to exact dimensions (e.g. 1024x1024)
    - `fit:WIDTHxHEIGHT`: scale keeping original aspect ratio and pad remaining space
  - Resampling filters: Lanczos3 (high quality), Triangle (Bilinear), Nearest Neighbor

- **Bidirectional Local Converter & Batch Processing**:
  - Convert PAA <-> PNG, JPG, TGA, BMP, WebP
  - Single file and directory batch conversion via CLI or GUI

---

## Build & Installation

Requires Rust 1.85+ (Edition 2024):

```bash
# Build pure Rust binary and libraries
cargo build --release
```

---

## Packaging

The project includes production-ready automated packaging scripts:

### 1. Debian Package (`.deb`)
Creates a complete `.deb` package containing both binaries, icons, desktop file, MIME database entry, and post-installation hooks:

```bash
./scripts/build-deb.sh
```
Output: `dist/paa-viewer_0.1.1_amd64.deb`

Install on Debian/Ubuntu/Mint:
```bash
sudo dpkg -i dist/paa-viewer_0.1.1_amd64.deb
```

### 2. Linux AppImage (`.AppImage`)
Creates a standalone, portable AppImage bundle that runs on any modern Linux distribution without installing dependencies:

```bash
./scripts/build-appimage.sh
```
Output: `dist/PAA_Viewer-x86_64.AppImage`

Run directly:
```bash
./dist/PAA_Viewer-x86_64.AppImage
```

### 3. Windows Executable & Bundle (`.exe` & `.zip`)
Cross-compiles for Windows x86_64 and packages the executable, icon, and context menu registry installer:

```bash
./scripts/build-windows.sh
```
Output: `dist/paa-viewer-windows-x64.zip`

---

## CLI Usage

### Launch Graphical Interface

```bash
# Launch default GUI (egui)
paa-viewer

# Launch GUI and open texture
paa-viewer texture.paa


```

### System Context Menu Management

```bash
# Register in system as default for double-click and context menus
paa-viewer install

# Or via submenu:
paa-viewer context-menu install

# Check registration status
paa-viewer context-menu status

# Unregister from context menu
paa-viewer context-menu uninstall

# Export Windows .reg file for manual installation
paa-viewer context-menu export-reg paa-context-menu.reg
```

### Inspect PAA Metadata (`info`)

```bash
# Human-readable table in terminal
paa-viewer info texture.paa

# JSON output for modding tools and automation
paa-viewer info texture.paa --json
```

### Convert Images (`convert`)

```bash
# Convert PNG to PAA (DXT5) with power-of-two alignment
paa-viewer convert input.png output.paa --format dxt5 --resize pot-nearest

# Convert PAA to PNG (output path auto-derived if omitted)
paa-viewer convert texture.paa

# Scale texture by 50% (half resolution)
paa-viewer convert texture.paa scaled.paa --resize 0.5
```

### Batch Conversion (`batch`)

```bash
# Batch convert directory of PNGs to PAA
paa-viewer batch ./input_png --out-dir ./output_paa --to paa --format dxt5 --resize pot-nearest

# Batch convert PAAs to PNG
paa-viewer batch ./input_paa --out-dir ./output_png --to png
```

---

## Project Structure

```
paa-viewer/
├── Cargo.toml            # Rust manifest (binary + library)
├── src/
│   ├── main.rs           # CLI entry point and dispatch
│   ├── lib.rs            # Public Rust library API
│   ├── context_menu.rs   # Linux & Windows context menu registration
│   ├── convert.rs        # Format conversion and batch processing
│   ├── resize.rs         # Power-of-two math and resampling
│   ├── gui/              # egui cross-platform desktop interface
│   └── paa/              # Core PAA codec (LZO, LZ77, Mipmaps, Tags, Pax)
├── packaging/
│   ├── paa-viewer.desktop
│   ├── paa-viewer.xml    # FreeDesktop MIME type image/x-paa
│   ├── paa-viewer.svg    # Vector application icon
│   ├── paa-viewer.png    # 256x256 raster icon
│   ├── deb/              # Debian package control files
│   ├── appimage/         # AppRun launcher script
│   └── windows/          # Registry script & .ico icon
├── scripts/
│   ├── build-deb.sh      # Debian packaging script
│   ├── build-appimage.sh # AppImage packaging script
│   └── build-windows.sh  # Windows cross-compilation & packaging script
├── tests/
│   └── paa_tests.rs      # Comprehensive codec and pipeline tests
└── .github/workflows/
    └── release.yml       # Automated GitHub Actions release pipeline
```

---

## License

GPL-2.0 / MIT
