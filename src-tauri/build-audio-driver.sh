#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=== Building Sendin Beats Audio Driver System ==="
echo ""

# Build the driver
echo "Building C AudioServerPlugIn driver..."
cd "${SCRIPT_DIR}/driver"
./build-driver.sh
echo ""

# Build the helper
echo "Building Swift privileged helper..."
cd "${SCRIPT_DIR}/helper"
./build-helper.sh
echo ""

echo "=== Build Complete ==="
echo "Driver bundle: ${SCRIPT_DIR}/bin/SendinBeatsAudioDriver.bundle"
echo "Helper binary: ${SCRIPT_DIR}/bin/sendin-beats-helper"
echo ""
echo "To install the driver, run:"
echo "  sudo ${SCRIPT_DIR}/bin/sendin-beats-helper install ${SCRIPT_DIR}/bin/SendinBeatsAudioDriver.bundle"
