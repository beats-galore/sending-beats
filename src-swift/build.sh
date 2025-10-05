#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="${SCRIPT_DIR}/build"
LIB_NAME="libscreencapture_audio"

echo "[Swift] Building ScreenCaptureAudio library..."

# Clean previous build
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"

# Compile Swift to object file
swiftc \
    -emit-library \
    -emit-module \
    -module-name ScreenCaptureAudio \
    -o "${BUILD_DIR}/${LIB_NAME}.dylib" \
    -Xlinker -install_name -Xlinker "@rpath/${LIB_NAME}.dylib" \
    "${SCRIPT_DIR}/ScreenCaptureAudio.swift"

# Generate static library as well
swiftc \
    -emit-library \
    -static \
    -emit-module \
    -module-name ScreenCaptureAudio \
    -o "${BUILD_DIR}/${LIB_NAME}.a" \
    "${SCRIPT_DIR}/ScreenCaptureAudio.swift"

echo "[Swift] Library built successfully:"
echo "  - ${BUILD_DIR}/${LIB_NAME}.dylib"
echo "  - ${BUILD_DIR}/${LIB_NAME}.a"
echo ""
echo "[Swift] To use in Rust, add to Cargo.toml:"
echo "  println!(\"cargo:rustc-link-search=native=${BUILD_DIR}\");"
echo "  println!(\"cargo:rustc-link-lib=static=screencapture_audio\");"
