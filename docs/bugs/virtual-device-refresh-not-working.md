## Virtual Device Refresh Not Working

**Status**: UNRESOLVED  
**Priority**: Medium  
**Date Discovered**: 2025-08-30

**Description**: When clicking the refresh button in the device selector, only system audio devices (microphones, speakers) are refreshed and updated in the UI. Virtual audio devices (like BlackHole, Loopback devices) do not appear in the updated list, even if they were recently installed or configured.

**Steps to Reproduce**:

1. Open the Virtual Mixer interface
2. Install or configure a new virtual audio device (e.g., BlackHole 16ch)
3. Click the refresh button in any device selector dropdown
4. Expected: All devices including virtual ones should appear
5. Actual: Only system devices refresh, virtual devices remain unchanged

**Investigation Done**:

- Device refresh functionality works for physical/system devices
- Backend device enumeration may not be properly detecting virtual devices on refresh
- Initial device loading may work differently than refresh logic

**Next Steps**:

- Investigate `refresh_audio_devices` command in `src-tauri/src/commands/audio_devices.rs:16`
- Check if `AudioDeviceManager` in `src-tauri/src/audio/devices/device_manager.rs` handles virtual devices differently on refresh vs initial load
- Verify Core Audio device enumeration includes virtual devices during refresh operations
- Test with different virtual audio drivers (BlackHole, Loopback, etc.)

**Workaround** (if any): Restart the application to detect newly added virtual devices