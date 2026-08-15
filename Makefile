# Native components that have to be compiled before the Tauri app can run.
#
# These produce artifacts the Rust side loads at runtime or ships as a bundled
# resource rather than links against, so they are built here instead of from
# build.rs.

.PHONY: native swift mediaremote driver clean-native

MEDIAREMOTE_DIR := src-mediaremote
MEDIAREMOTE_BUILD := $(MEDIAREMOTE_DIR)/build
DRIVER_DIR := src-driver

native: swift mediaremote driver

# ScreenCaptureKit audio capture, loaded through the FFI in audio/screencapture.
swift:
	@./src-swift/build.sh

# MediaRemote adapter: a helper framework that /usr/bin/perl loads to read the
# system now-playing session. Reading it needs an entitlement Apple grants only
# to its own binaries, so the query has to run out of a platform binary rather
# than out of this app.
mediaremote:
	@echo "[MediaRemote] Building adapter framework..."
	@cmake -B $(MEDIAREMOTE_BUILD) -S $(MEDIAREMOTE_DIR) -DCMAKE_BUILD_TYPE=Release > /dev/null
	@cmake --build $(MEDIAREMOTE_BUILD) --target MediaRemoteAdapter > /dev/null
	@echo "[MediaRemote] Built $(MEDIAREMOTE_BUILD)/MediaRemoteAdapter.framework"

# HAL virtual audio driver. tauri.conf.json ships the built bundle as a
# resource, so the Tauri build fails outright if this has not run. Installing it
# into /Library/Audio/Plug-Ins/HAL needs sudo and stays a separate step.
driver:
	@$(MAKE) -C $(DRIVER_DIR)

clean-native:
	rm -rf $(MEDIAREMOTE_BUILD) src-swift/build $(DRIVER_DIR)/build
