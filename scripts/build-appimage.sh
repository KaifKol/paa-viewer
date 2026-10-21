#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== Building PAA Viewer AppImage / Portable Linux Bundle ==="

cd "${ROOT_DIR}"

# 1. Compile Rust release binary & library
cargo build --release

# 2. Compile Qt frontend if cmake available
if command -v cmake >/dev/null 2>&1; then
    cmake -B qt/build -S qt
    cmake --build qt/build --target paa-viewer-qt || true
fi

# 3. Create AppDir structure
APPDIR="${ROOT_DIR}/dist/AppDir"
rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr/bin"
mkdir -p "${APPDIR}/usr/lib"
mkdir -p "${APPDIR}/usr/share/applications"
mkdir -p "${APPDIR}/usr/share/mime/packages"
mkdir -p "${APPDIR}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${APPDIR}/usr/share/icons/hicolor/256x256/apps"

# Copy AppRun, desktop file, icon
cp packaging/appimage/AppRun "${APPDIR}/"
chmod 0755 "${APPDIR}/AppRun"

cp packaging/paa-viewer.desktop "${APPDIR}/"
cp packaging/paa-viewer.desktop "${APPDIR}/usr/share/applications/"
cp packaging/paa-viewer.png "${APPDIR}/"
cp packaging/paa-viewer.svg "${APPDIR}/"
cp packaging/paa-viewer.svg "${APPDIR}/usr/share/icons/hicolor/scalable/apps/"
cp packaging/paa-viewer.png "${APPDIR}/usr/share/icons/hicolor/256x256/apps/"
cp packaging/paa-viewer.xml "${APPDIR}/usr/share/mime/packages/"

# Copy binaries
cp target/release/paa-viewer "${APPDIR}/usr/bin/"
chmod 0755 "${APPDIR}/usr/bin/paa-viewer"

if [ -f qt/build/paa-viewer-qt ]; then
    cp qt/build/paa-viewer-qt "${APPDIR}/usr/bin/"
    chmod 0755 "${APPDIR}/usr/bin/paa-viewer-qt"
fi

if [ -f target/release/libpaa_viewer.so ]; then
    cp target/release/libpaa_viewer.so "${APPDIR}/usr/lib/"
    chmod 0755 "${APPDIR}/usr/lib/libpaa_viewer.so"
fi

mkdir -p "${ROOT_DIR}/dist"
OUTPUT_APPIMAGE="${ROOT_DIR}/dist/PAA_Viewer-x86_64.AppImage"

# Check if appimagetool is available or download it
APPIMAGETOOL=""
if command -v appimagetool >/dev/null 2>&1; then
    APPIMAGETOOL="appimagetool"
elif [ -x "/tmp/appimagetool" ]; then
    APPIMAGETOOL="/tmp/appimagetool"
fi

if [ -z "${APPIMAGETOOL}" ]; then
    echo "Downloading appimagetool..."
    if curl -fsSL -o /tmp/appimagetool https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage; then
        chmod +x /tmp/appimagetool
        APPIMAGETOOL="/tmp/appimagetool"
    fi
fi

if [ -n "${APPIMAGETOOL}" ]; then
    echo "Creating AppImage with ${APPIMAGETOOL}..."
    ARCH=x86_64 "${APPIMAGETOOL}" --appimage-extract-and-run "${APPDIR}" "${OUTPUT_APPIMAGE}" || {
        echo "appimagetool execution failed, falling back to standalone portable bundle..."
    }
fi

# Also create portable standalone tarball
PORTABLE_TAR="${ROOT_DIR}/dist/paa-viewer-linux-x86_64.tar.gz"
echo "Creating portable Linux tarball: ${PORTABLE_TAR}..."
tar -czf "${PORTABLE_TAR}" -C "${ROOT_DIR}/dist" AppDir

echo "Linux packaging complete!"
if [ -f "${OUTPUT_APPIMAGE}" ]; then
    ls -lh "${OUTPUT_APPIMAGE}"
fi
ls -lh "${PORTABLE_TAR}"
