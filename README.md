# PAA Viewer & Converter

Cross-platform texture viewer, converter, and power-of-two (POT) resizer for Bohemia Interactive PAA files (Arma 3, DayZ, Real Virtuality engine), written in Rust.

## Features

- **Desktop GUI Viewer (egui / eframe)**:
  - High-performance GPU-accelerated rendering with smooth zoom and pan
  - Checkerboard background for transparent alpha textures
  - RGBA channel inspection: view Full RGBA, RGB only, individual Red, Green, Blue channels, or Alpha as grayscale
  - Real-time pixel inspector: hover cursor over any pixel to view (X, Y) coordinates, RGBA values, hex code `#RRGGBBAA`, and color swatch
  - Interactive Mipmap Navigator: inspect all mip levels with dimensions, compressed vs uncompressed size, compression ratio, and switch view to any level instantly
  - Drag & Drop: drag any `.paa`, `.png`, `.jpg`, `.tga`, `.bmp`, or `.webp` into the window to load immediately
  - Native file open and export dialogs (`rfd`)

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

## Installation & Build

Requires Rust 1.85+ (Edition 2024):

```bash
cargo build --release
```

The compiled binary will be located at `target/release/paa-viewer`.

---

## CLI Usage

### 1. Launch GUI

```bash
# Launch GUI directly
paa-viewer

# Launch GUI and immediately open a texture
paa-viewer path/to/texture.paa
```

### 2. Inspect PAA Metadata (`info`)

Print complete technical details, mipmap hierarchy, and tagged sections:

```bash
paa-viewer info texture.paa
```

Output formatted as JSON (useful for modding tools and scripting):

```bash
paa-viewer info texture.paa --json
```

### 3. Convert Single Image (`convert`)

Convert PNG to PAA with DXT5 compression, power-of-two alignment, and mipmap generation:

```bash
paa-viewer convert input.png output.paa --format dxt5 --resize pot-nearest
```

Convert PAA to PNG:

```bash
paa-viewer convert input.paa output.png
```

Scale texture by 50% (half resolution):

```bash
paa-viewer convert input.paa output.paa --resize 0.5
```

Resize to exact 1024x1024 with Lanczos3 filter:

```bash
paa-viewer convert input.png output.paa --format dxt1 --resize 1024x1024 --filter lanczos
```

Available format options: `dxt1`, `dxt3`, `dxt5`, `argb8888`, `argb1555`, `argb4444`, `graya`.

### 4. Batch Conversion (`batch`)

Batch convert an entire directory of PNGs to PAA:

```bash
paa-viewer batch ./textures_png --out-dir ./textures_paa --to paa --format dxt5 --resize pot-nearest
```

Batch convert PAAs to PNGs:

```bash
paa-viewer batch ./textures_paa --out-dir ./textures_png --to png
```

---

## Architecture & Codebase Design

- `src/paa/`: Core PAA codec module
  - `pax.rs`: `PaXType` definitions, pixel converters, `texpresso` bridge
  - `lzo.rs`: LZO1X-1 / LZSS compression and decompressor
  - `lz77.rs`: Bohemia LZ77 compression and decompressor
  - `mipmap.rs`: `Mipmap` level parsing, encoding, and RGBA decoding
  - `taggs.rs`: `GGAT` tagged metadata sections parser and writer
  - `image.rs`: `PaaImage` top-level container, reader, writer, and metadata extractor
- `src/resize.rs`: Power-of-two mathematical routines, factor scaling, stretch, and fit-pad logic
- `src/convert.rs`: File format conversion pipeline and batch processing engine
- `src/cli.rs`: Command line arguments definition and parsing
- `src/gui/`: Graphical interface implementation
  - `app.rs`: Main egui viewport, canvas drawing, channel modes, inspector, and sidebar controls
  - `mod.rs`: Native desktop window initialization via `eframe`

---

## License

GPL-2.0 / MIT
