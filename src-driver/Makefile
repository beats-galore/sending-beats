# Makefile for Sendin Beats Audio Driver
# Based on BlackHole audio loopback driver

DRIVER_NAME = SendinBeatsAudio
BUNDLE_ID = com.sendinbeats.audio.driver
DEVICE_NAME = Sendin\ Beats\ Audio
MANUFACTURER = Sendin\ Beats
CHANNELS = 2

# Build paths
SRC = SendinBeatsAudio.c
BUILD_DIR = build
BUNDLE_DIR = $(BUILD_DIR)/$(DRIVER_NAME).driver
CONTENTS_DIR = $(BUNDLE_DIR)/Contents
MACOS_DIR = $(CONTENTS_DIR)/MacOS
RESOURCES_DIR = $(CONTENTS_DIR)/Resources

# Compiler flags
CC = clang
CFLAGS = -O2 -Wall -Wextra \
	-framework CoreAudio \
	-framework CoreFoundation \
	-framework Accelerate \
	-DkDriver_Name=\"$(DRIVER_NAME)\" \
	-DkPlugIn_BundleID=\"$(BUNDLE_ID)\" \
	-DkDevice_Name=\"$(DEVICE_NAME)\" \
	-DkManufacturer_Name=\"$(MANUFACTURER)\" \
	-DkNumber_Of_Channels=$(CHANNELS) \
	-DkDevice_IsHidden=false \
	-DkCanBeDefaultDevice=true \
	-DkCanBeDefaultSystemDevice=true \
	-bundle

.PHONY: all clean install uninstall

all: $(BUNDLE_DIR)

$(BUNDLE_DIR): $(SRC) Info.plist
	@echo "Building $(DRIVER_NAME).driver..."
	@mkdir -p $(MACOS_DIR)
	@mkdir -p $(RESOURCES_DIR)

	@echo "Compiling driver..."
	$(CC) $(CFLAGS) $(SRC) -o $(MACOS_DIR)/$(DRIVER_NAME)

	@echo "Copying Info.plist..."
	@cp Info.plist $(CONTENTS_DIR)/Info.plist

	@echo "Build complete: $(BUNDLE_DIR)"

clean:
	@echo "Cleaning build directory..."
	@rm -rf $(BUILD_DIR)

install: $(BUNDLE_DIR)
	@echo "Installing $(DRIVER_NAME).driver..."
	@echo "This requires sudo privileges"
	@sudo mkdir -p /Library/Audio/Plug-Ins/HAL
	@sudo cp -R $(BUNDLE_DIR) /Library/Audio/Plug-Ins/HAL/
	@echo "Driver installed. Restart coreaudiod:"
	@echo "  sudo launchctl kickstart -kp system/com.apple.audio.coreaudiod"

uninstall:
	@echo "Uninstalling $(DRIVER_NAME).driver..."
	@sudo rm -rf /Library/Audio/Plug-Ins/HAL/$(DRIVER_NAME).driver
	@echo "Driver uninstalled. Restart coreaudiod:"
	@echo "  sudo launchctl kickstart -kp system/com.apple.audio.coreaudiod"
