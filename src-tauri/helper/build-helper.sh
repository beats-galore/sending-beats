#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HELPER_NAME="sendin-beats-helper"
BUILD_DIR="${SCRIPT_DIR}/build"
BIN_DIR="${SCRIPT_DIR}/../bin"

echo "[Build] Building ${HELPER_NAME}..."

# Clean previous build
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"
mkdir -p "${BIN_DIR}"

# Compile Swift helper
echo "[Build] Compiling Swift helper..."
swiftc \
    -o "${BUILD_DIR}/${HELPER_NAME}" \
    "${SCRIPT_DIR}/SendinBeatsHelper.swift"

# Copy to bin directory for Tauri bundling
cp "${BUILD_DIR}/${HELPER_NAME}" "${BIN_DIR}/"

echo "[Build] Helper built successfully at ${BUILD_DIR}/${HELPER_NAME}"
echo "[Build] Copied to ${BIN_DIR}/${HELPER_NAME}"

# Check if we should sign
if [ -n "${CODESIGN_IDENTITY}" ]; then
    echo "[Build] Code signing with identity: ${CODESIGN_IDENTITY}"
    codesign --force --sign "${CODESIGN_IDENTITY}" "${BIN_DIR}/${HELPER_NAME}"
    echo "[Build] Code signing complete"
else
    echo "[Build] WARNING: CODESIGN_IDENTITY not set, skipping code signing"
    echo "[Build] Set CODESIGN_IDENTITY environment variable to enable signing"
fi

echo "[Build] Build complete!"
