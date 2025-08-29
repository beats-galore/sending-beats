## Audio Mixer Issues

### Master Output Gain Control Non-Functional

**Status**: UNRESOLVED  
**Priority**: High  
**Date Discovered**: 2025-08-26

**Description**: The master output gain control in the mixer UI does not affect
the actual audio output volume. The slider moves but no audio level changes
occur.

**Investigation Done**:

- Issue reported by user during testing

**Next Steps**:

1. Check if master gain control is connected to backend audio processing
2. Verify Tauri command for master gain exists and is called
3. Test audio pipeline to ensure master gain is applied in the signal chain
4. Check if master gain is being overridden elsewhere in the audio flow

**Workaround**: Use individual channel gain controls or system volume controls.
