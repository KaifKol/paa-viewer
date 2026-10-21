#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== Building PAA Viewer Debian Package (.deb) ==="

cd "${ROOT_DIR}"

# 1. Compile Rust release binary & library
echo "Compiling Rust release artifacts..."
cargo build --release

# 2. Compile Qt frontend if available
if command -v cmake >/dev/null 2>&1; then
    echo "Compiling Qt6 frontend..."
    cmake -B qt/build -S qt
    cmake --build qt/build --target paa-viewer-qt
fi

# 3. Create staging directory
STAGE_DIR=$(mktemp -d /tmp/paa-deb-build.XXXXXX)
trap 'rm -rf "${STAGE_DIR}"' EXIT

mkdir -p "${STAGE_DIR}/DEBIAN"
mkdir -p "${STAGE_DIR}/usr/bin"
mkdir -p "${STAGE_DIR}/usr/lib"
mkdir -p "${STAGE_DIR}/usr/share/applications"
mkdir -p "${STAGE_DIR}/usr/share/mime/packages"
mkdir -p "${STAGE_DIR}/usr/share/kio/servicemenus"
mkdir -p "${STAGE_DIR}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${STAGE_DIR}/usr/share/icons/hicolor/256x256/apps"

# Copy control files
cp packaging/deb/control "${STAGE_DIR}/DEBIAN/"
cp packaging/deb/postinst "${STAGE_DIR}/DEBIAN/"
cp packaging/deb/postrm "${STAGE_DIR}/DEBIAN/"
chmod 0755 "${STAGE_DIR}/DEBIAN/postinst"
chmod 0755 "${STAGE_DIR}/DEBIAN/postrm"

# Copy binaries & libraries
cp target/release/paa-viewer "${STAGE_DIR}/usr/bin/"
chmod 0755 "${STAGE_DIR}/usr/bin/paa-viewer"

if [ -f qt/build/paa-viewer-qt ]; then
    cp qt/build/paa-viewer-qt "${STAGE_DIR}/usr/bin/"
    chmod 0755 "${STAGE_DIR}/usr/bin/paa-viewer-qt"
fi

if [ -f target/release/libpaa_viewer.so ]; then
    cp target/release/libpaa_viewer.so "${STAGE_DIR}/usr/lib/"
    chmod 0755 "${STAGE_DIR}/usr/lib/libpaa_viewer.so"
fi

# Copy desktop, mime, and icons
cp packaging/paa-viewer.desktop "${STAGE_DIR}/usr/share/applications/"
cp packaging/paa-viewer.xml "${STAGE_DIR}/usr/share/mime/packages/"
cp packaging/paa-viewer.svg "${STAGE_DIR}/usr/share/icons/hicolor/scalable/apps/"
cp packaging/paa-viewer.png "${STAGE_DIR}/usr/share/icons/hicolor/256x256/apps/"

mkdir -p "${ROOT_DIR}/dist"
OUTPUT_DEB="${ROOT_DIR}/dist/paa-viewer_0.1.0_amd64.deb"

# 4. Pack into .deb archive
if command -v dpkg-deb >/dev/null 2>&1; then
    dpkg-deb --build --root-owner-group "${STAGE_DIR}" "${OUTPUT_DEB}"
else
    echo "Using standard tar + ar to create Debian package..."
    cd "${STAGE_DIR}"
    echo "2.0" > debian-binary

    cd "${STAGE_DIR}/DEBIAN"
    tar -czf "${STAGE_DIR}/control.tar.gz" --owner=0 --group=0 *

    cd "${STAGE_DIR}"
    tar -czf "${STAGE_DIR}/data.tar.gz" --owner=0 --group=0 --exclude="./DEBIAN" --exclude="./debian-binary" --exclude="./control.tar.gz" ./usr

    ar rcs "${OUTPUT_DEB}" debian-binary control.tar.gz data.tar.gz
fi

echo "Debian package successfully created at: ${OUTPUT_DEB}"
ls -lh "${OUTPUT_DEB}"
