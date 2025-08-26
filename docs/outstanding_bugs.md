# Outstanding Bugs and Known Issues

This document tracks bugs and issues we've discovered that need to be addressed in future development sessions.

## Audio Recording Issues

### WAV File Compatibility Issue
**Status**: UNRESOLVED  
**Priority**: Medium  
**Date Discovered**: 2025-08-26

**Description**:
WAV files recorded by the application can only be opened in VLC player, but fail to open in iTunes, QuickTime, and other standard audio players on macOS.

**Investigation Done**:
- Changed default bit depth from 24-bit to 16-bit (most compatible format)
- Updated both backend (`src-tauri/src/recording_service.rs:101`) and frontend (`src/components/dj/RecordingControlsCard.tsx:192`) defaults
- WAV encoder uses standard PCM format (format code 1) with correct header structure

**Potential Causes**:
1. WAV header corruption or incorrect size calculations
2. Missing or incorrect metadata chunks
3. Byte ordering issues in the header
4. File size updates not being written correctly during finalization
5. Audio sample encoding issues (though VLC plays them fine)

**Next Steps**:
1. Generate a test WAV file and analyze with hex editor
2. Compare header structure with known-good WAV files
3. Test with different audio applications to narrow down compatibility issue
4. Consider using a proven WAV library instead of custom implementation
5. Add WAV file validation/verification after recording

**Workaround**: 
Users can convert files using VLC or other audio converters, or use MP3 format instead.

---

## Audio Mixer Issues

### Master Output Gain Control Non-Functional
**Status**: UNRESOLVED  
**Priority**: High  
**Date Discovered**: 2025-08-26

**Description**:
The master output gain control in the mixer UI does not affect the actual audio output volume. The slider moves but no audio level changes occur.

**Investigation Done**:
- Issue reported by user during testing

**Next Steps**:
1. Check if master gain control is connected to backend audio processing
2. Verify Tauri command for master gain exists and is called
3. Test audio pipeline to ensure master gain is applied in the signal chain
4. Check if master gain is being overridden elsewhere in the audio flow

**Workaround**: 
Use individual channel gain controls or system volume controls.

### Equalizer Effect Shows "No Effects Added" 
**Status**: UNRESOLVED  
**Priority**: Medium  
**Date Discovered**: 2025-08-26

**Description**:
When adding an equalizer effect to a channel, the UI still displays "No effects added" instead of showing the equalizer controls.

**Investigation Done**:
- Issue reported by user during testing

**Next Steps**:
1. Check effect addition logic in frontend components
2. Verify effect state management and UI updates
3. Test if effect is actually being applied to audio (might be UI-only issue)
4. Check effect component rendering conditions

**Workaround**: 
Effect may still be working despite UI not showing it - test with audio.

### Cannot Re-add Effects After Removal
**Status**: UNRESOLVED  
**Priority**: Medium  
**Date Discovered**: 2025-08-26

**Description**:
Once an effect is removed from a channel, it cannot be re-added. The effect option becomes unavailable or non-functional.

**Investigation Done**:
- Issue reported by user during testing

**Next Steps**:
1. Check effect removal logic and state cleanup
2. Verify if removed effects are properly reset/reinitialized 
3. Test effect availability tracking in UI state
4. Check for memory leaks or incomplete cleanup preventing re-addition

**Workaround**: 
Restart application to reset effect availability.

### Output Source Change Crashes Application
**Status**: UNRESOLVED  
**Priority**: High  
**Date Discovered**: 2025-08-26

**Description**:
Changing the audio output source after it has been initially selected causes the entire application to crash.

**Investigation Done**:
- Issue reported by user during testing
- This is a regression or persistent issue that affects core audio functionality

**Next Steps**:
1. Check audio device switching logic in backend
2. Look for resource cleanup issues when switching output devices
3. Test with different output devices (speakers, headphones, virtual devices)
4. Add proper error handling and device switching safety measures
5. Check if this is related to previous audio device management fixes

**Workaround**: 
Set correct output device before starting audio, avoid changing after initialization.

### Audio Stream Crunchiness on Bass Frequencies
**Status**: UNRESOLVED  
**Priority**: Medium  
**Date Discovered**: 2025-08-26

**Description**:
Despite significant improvements to audio stream quality, there are still occasional crunches and glitches that occur specifically on bass frequencies. The audio stream has gotten much better overall but bass-heavy content still experiences intermittent distortion.

**Investigation Done**:
- Previous fixes have significantly improved audio quality
- Issue is now isolated to bass frequency range
- Problem is intermittent rather than constant

**Next Steps**:
1. Check low-frequency filter and processing in audio effects chain
2. Verify bass EQ band processing for clipping or overflow issues
3. Test with different bass-heavy audio sources to isolate trigger conditions
4. Check compressor/limiter settings - may be too aggressive on low frequencies
5. Analyze buffer sizes and sample processing for low-frequency artifacts
6. Consider if bass frequencies need different processing parameters
7. Test with different bit depths and sample rates for bass content

**Workaround**: 
Reduce bass levels in EQ or use external audio processing before input.

---

## Template for New Bugs

### Bug Title
**Status**: [UNRESOLVED/IN_PROGRESS/RESOLVED]  
**Priority**: [High/Medium/Low]  
**Date Discovered**: YYYY-MM-DD

**Description**:
Clear description of the issue

**Steps to Reproduce**:
1. Step 1
2. Step 2
3. Expected vs actual behavior

**Investigation Done**:
- What has been tried
- What was ruled out

**Next Steps**:
- Specific actions to take
- Files/functions to investigate

**Workaround** (if any):
Alternative approach for users