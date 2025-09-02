## Application Crashes When Audio Output Device is Unplugged

**Status**: UNRESOLVED  
**Priority**: High  
**Date Discovered**: 2025-09-02

**Description**: The application crashes or becomes unresponsive when an audio output device that is currently configured as the active output source is physically unplugged or disconnected while the mixer is running.

**Steps to Reproduce**:

1. Start the mixer application
2. Configure a USB/Bluetooth audio device (like JBL TUNE770NC headphones) as the output source
3. Begin audio playback through the mixer
4. Physically unplug or disconnect the configured output device
5. Application crashes or becomes unresponsive with CoreAudio errors

**Expected vs Actual Behavior**:
- **Expected**: Application should gracefully handle device disconnection, show an error message, and either switch to a fallback device or pause audio output
- **Actual**: Application crashes or freezes, potentially with CoreAudio error -10851 or similar device access errors

**Investigation Done**:

- CoreAudio error -10851 typically indicates "Audio Unit not set up correctly"
- Error occurs when trying to access a device that no longer exists
- Current device selection logic doesn't handle device disappearance gracefully
- Stream operations may continue trying to send audio to non-existent device

**Next Steps**:

- Implement device disconnection detection in CoreAudio stream management
- Add device health monitoring to detect when devices become unavailable
- Implement graceful fallback to default system audio device
- Add UI notifications for device disconnection events
- Review error handling in `src-tauri/src/audio/devices/coreaudio_stream.rs`
- Review device management in `src-tauri/src/audio/devices/device_manager.rs`

**Workaround** (if any): 
- Stop the mixer before disconnecting audio devices
- Restart the application if it becomes unresponsive after device disconnection
- Configure a stable system audio device (like built-in speakers) as output before unplugging external devices

**Related Code Areas**:
- `src-tauri/src/audio/devices/coreaudio_stream.rs` - CoreAudio stream management
- `src-tauri/src/audio/devices/device_manager.rs` - Device health monitoring
- `src-tauri/src/audio/mixer/stream_operations.rs` - Stream operations and device switching