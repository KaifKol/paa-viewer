# PAA Viewer

A desktop viewer and converter for PAA textures used by Arma 3, DayZ, and
other Bohemia Interactive games.

Use it to inspect a texture without starting the game, convert common image
formats to PAA, or prepare power-of-two textures for a mod.

## Download

Prebuilt packages are available on the
[Releases page](https://github.com/KaifKol/paa-viewer/releases/latest).

| Platform | Package |
| --- | --- |
| Windows x64 | `paa-viewer-windows-x64.zip` |
| Debian, Ubuntu, Linux Mint | `.deb` |
| Other x64 Linux distributions | `.AppImage` or portable `.tar.gz` |

On Windows, extract the archive and run `paa-viewer.exe`.

On Linux, make the AppImage executable before starting it:

```bash
chmod +x PAA_Viewer-x86_64.AppImage
./PAA_Viewer-x86_64.AppImage
```

## Features

- Open `.paa`, `.pac`, `.png`, `.jpg`, `.jpeg`, `.tga`, `.bmp`, and `.webp`
- Inspect PAA metadata, tags, mipmap levels, and pixel RGBA values
- View RGB, alpha, or individual color channels
- Zoom, pan, fit to window, and switch between nearest and linear filtering
- Export images to PAA or a common image format
- Generate PAA mipmaps automatically
- Resize by a scale factor or align each dimension to a power of two
- Convert one file or a directory from the command line
- Configure whether externally launched files reuse the current viewer or open in a separate window
- Add file associations, thumbnail previews, and conversion actions to Windows Explorer or Linux
  file managers

## Using the desktop app

Run the program without arguments:

```bash
paa-viewer
```

You can also open a file directly:

```bash
paa-viewer texture.paa
```

Drop a file into the window or use **Open File**. The right panel shows PAA
metadata and mipmap levels. Scroll over the image to zoom and drag to pan.
Choose the output format and mipmap settings under **PAA Export Settings**,
then use **Export As**.
Resize actions always operate on the base image, even when a lower mipmap level
is selected for preview.

### Settings and window behavior

Open **Settings** from the top toolbar to configure application preferences:

- **External file open mode:** Choose whether files launched from Windows Explorer, Linux file managers, or the command line reuse the running viewer window or open in a separate native window. (In-app **Open File** and drag-and-drop always display the file in the current window.)
- **System Integration:** Register or remove file associations, thumbnail preview handlers, and desktop context menu actions.

## Command line

### Inspect a PAA file

```bash
paa-viewer info texture.paa
paa-viewer info texture.paa --json
```

### Convert files

If the output path is omitted, PAA and PAC files are converted to PNG. Other
images are converted to PAA.

```bash
# PAA to PNG
paa-viewer convert texture.paa

# PNG to DXT5 PAA
paa-viewer convert source.png texture.paa --format dxt5

# Convert and round each dimension to the nearest power of two
paa-viewer convert source.png texture.paa --format dxt5 --resize pot-nearest

# Resize to an exact size
paa-viewer convert source.png texture.paa --resize 1024x1024

# Fit inside a size while preserving aspect ratio and pad the remaining area
paa-viewer convert source.png texture.paa --resize fit:1024x1024
```

Available resize modes:

| Value | Result |
| --- | --- |
| `0.5`, `2.0` | Scale by a factor |
| `pot-nearest` | Round each dimension to the nearest power of two |
| `pot-next` | Round each dimension up |
| `pot-prev` | Round each dimension down |
| `WIDTHxHEIGHT` | Stretch to an exact size |
| `fit:WIDTHxHEIGHT` | Preserve aspect ratio and pad to the requested size |

Use `--filter lanczos`, `--filter triangle`, or `--filter nearest` to select
the resampling filter.

### Convert a directory

```bash
# PNG and other supported images to PAA
paa-viewer batch ./source --out-dir ./paa --to paa \
  --format dxt5 --resize pot-nearest

# PAA files to PNG, including subdirectories
paa-viewer batch ./paa --out-dir ./png --to png --recursive
```

Run `paa-viewer --help` or `paa-viewer <command> --help` for the complete
option list.

## Supported formats

| Format | Read | Write |
| --- | :---: | :---: |
| PAA/PAC DXT1 | Yes | Yes |
| PAA/PAC DXT3 | Yes | Yes |
| PAA/PAC DXT5 | Yes | Yes |
| PAA/PAC ARGB8888 | Yes | Yes |
| PAA/PAC ARGB1555 | Yes | Yes |
| PAA/PAC ARGB4444 | Yes | Yes |
| PAA/PAC GRAYA | Yes | Yes |
| PNG, JPEG, TGA, BMP, WebP | Yes | Yes |

PAA files may contain LZO-compressed DXT mipmaps or LZ77-compressed ARGB
mipmaps. The viewer reads and writes both forms.

## File associations, context menus, and thumbnail previews

The app can register itself as the handler for `.paa` and `.pac` files, providing file associations, right-click conversion actions, and thumbnail previews in Windows Explorer and Linux FreeDesktop file managers.

Thumbnail previews and file associations are enabled by **System Integration**. Note that file-manager thumbnail caches may need to be refreshed or the file manager reopened to display newly generated previews.

Open **Settings** from the top toolbar in the desktop app, or run:

```bash
paa-viewer install
paa-viewer context-menu status
paa-viewer uninstall
```

Registration is per user on Linux. Windows registration is written to the
current user's registry.

## Build from source

Rust 1.85 or newer is required.

```bash
git clone https://github.com/KaifKol/paa-viewer.git
cd paa-viewer
cargo build --release
```

The binary is written to `target/release/paa-viewer`.

For Windows Explorer thumbnails, also build the Shell extension:

```bash
CARGO_TARGET_DIR=target cargo build --release \
  --manifest-path crates/windows-thumbnail-provider/Cargo.toml
```

Keep `paa_thumbnail_provider.dll` next to `paa-viewer.exe`, then run
`paa-viewer.exe install`.

Linux builds require the OpenGL and X11 development packages used by `eframe`.
On Debian and Ubuntu:

```bash
sudo apt install libgl1-mesa-dev libx11-dev libxcursor-dev \
  libxrandr-dev libxi-dev libxinerama-dev
```

To build distribution packages:

```bash
./scripts/build-deb.sh
./scripts/build-appimage.sh
./scripts/build-windows.sh
```

## License

[MIT](LICENSE)
