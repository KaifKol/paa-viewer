#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== Building PAA Viewer for Windows (x86_64) ==="

cd "${ROOT_DIR}"

TARGET="x86_64-pc-windows-gnu"

# Check if target is installed, add if needed
if ! rustup target list --installed | grep -q "${TARGET}"; then
    echo "Adding rustup target ${TARGET}..."
    rustup target add "${TARGET}" || true
fi

# Build for Windows
echo "Compiling for ${TARGET}..."
if cargo build --target "${TARGET}" --release; then
    EXE_SOURCE="target/${TARGET}/release/paa-viewer.exe"
else
    echo "Cross-compilation using ${TARGET} failed. If running in CI or on Windows, ensure MinGW or MSVC is installed."
    exit 1
fi

mkdir -p "${ROOT_DIR}/dist"
STAGE_DIR=$(mktemp -d /tmp/paa-win-build.XXXXXX)
trap 'rm -rf "${STAGE_DIR}"' EXIT

mkdir -p "${STAGE_DIR}/paa-viewer"
cp "${EXE_SOURCE}" "${STAGE_DIR}/paa-viewer/"
cp packaging/windows/paa-viewer-context-menu.reg "${STAGE_DIR}/paa-viewer/"
cp packaging/paa-viewer.ico "${STAGE_DIR}/paa-viewer/"
cp README.md "${STAGE_DIR}/paa-viewer/"

# Add Windows quick install batch script
cat << 'EOF' > "${STAGE_DIR}/paa-viewer/install-context-menu.bat"
@echo off
echo Registering PAA Viewer context menu and file associations...
reg import paa-viewer-context-menu.reg
if %ERRORLEVEL% EQU 0 (
    echo Successfully registered PAA Viewer in Windows context menu!
) else (
    echo Failed to import registry keys. Please right-click install-context-menu.bat and select 'Run as Administrator'.
)
pause
EOF

ZIP_OUTPUT="${ROOT_DIR}/dist/paa-viewer-windows-x64.zip"
echo "Creating Windows zip bundle: ${ZIP_OUTPUT}..."
(cd "${STAGE_DIR}" && zip -r "${ZIP_OUTPUT}" paa-viewer)

echo "Windows build complete!"
ls -lh "${ZIP_OUTPUT}"
