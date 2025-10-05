#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DRIVER_NAME="SendinBeatsAudioDriver"
BUNDLE_NAME="${DRIVER_NAME}.bundle"
BUILD_DIR="${SCRIPT_DIR}/build"
BUNDLE_DIR="${BUILD_DIR}/${BUNDLE_NAME}"

echo "[Build] Building ${DRIVER_NAME}..."

# Clean previous build
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"

# Create bundle directory structure
mkdir -p "${BUNDLE_DIR}/Contents/MacOS"
mkdir -p "${BUNDLE_DIR}/Contents/Resources"

# Compile the driver
echo "[Build] Compiling driver..."
clang -bundle \
    -framework CoreAudio \
    -framework CoreFoundation \
    -o "${BUNDLE_DIR}/Contents/MacOS/${DRIVER_NAME}" \
    "${SCRIPT_DIR}/${DRIVER_NAME}.c"

# Create Info.plist
echo "[Build] Creating Info.plist..."
cat > "${BUNDLE_DIR}/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>${DRIVER_NAME}</string>
    <key>CFBundleIdentifier</key>
    <string>com.sendinbeats.audiodriver</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${DRIVER_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>AudioServerPlugIn_PlugInClassName</key>
    <string>AudioDriverPlugInOpen</string>
</dict>
</plist>
EOF

# Copy to bin directory for Tauri bundling
BIN_DIR="${SCRIPT_DIR}/../bin"
mkdir -p "${BIN_DIR}"
cp -R "${BUNDLE_DIR}" "${BIN_DIR}/"

echo "[Build] Driver built successfully at ${BUNDLE_DIR}"
echo "[Build] Copied to ${BIN_DIR}/${BUNDLE_NAME}"

# Check if we should sign
if [ -n "${CODESIGN_IDENTITY}" ]; then
    echo "[Build] Code signing with identity: ${CODESIGN_IDENTITY}"
    codesign --force --sign "${CODESIGN_IDENTITY}" "${BIN_DIR}/${BUNDLE_NAME}"
    echo "[Build] Code signing complete"
else
    echo "[Build] WARNING: CODESIGN_IDENTITY not set, skipping code signing"
    echo "[Build] Set CODESIGN_IDENTITY environment variable to enable signing"
fi

echo "[Build] Build complete!"
