## Effects Chain Severely Limiting Audio Output

**Status**: UNRESOLVED  
**Priority**: High  
**Date Discovered**: 2025-09-11

**Description**: The audio effects chain is causing a ~10-40x reduction in audio signal levels, making output barely audible even with strong input signals. Default effects processing is being applied even when no effects are explicitly configured by the user.

**Steps to Reproduce**:

1. Start audio mixer with input device (e.g., BlackHole 2ch)
2. Play audio through input device with normal levels (peak: 0.3-0.7)
3. Observe output levels are severely reduced (peak: 0.01-0.03)
4. Audio is barely audible despite strong input signal

**Investigation Done**:

- Confirmed issue is in `process_with_effects()` function in audio_input_stream.rs
- Input levels: 0.2940-0.7213 (normal, strong signals)  
- After effects processing: 0.0104 (40x reduction!)
- After mixing: 0.0167 (still very low)
- Temporarily disabling effects (`effects_enabled = false`) resolves the issue
- Problem occurs even with default AudioChannel configuration

**Root Causes Identified**:

1. **Default Effects Applied**: Effects chain processes audio even when no effects are explicitly added by user
2. **Aggressive Default Settings**: Default EQ/compressor/limiter settings are too aggressive, crushing signal
3. **No Bypass Logic**: Effects chain doesn't have proper bypass when no effects are configured

**Next Steps**:

- Investigate default AudioChannel effects configuration in audio/types.rs
- Check ThreeBandEqualizer, Compressor, and Limiter default settings
- Implement proper effects bypass when no effects are explicitly enabled
- Ensure effects chain only processes effects that were explicitly configured via UI
- Add gain compensation to effects processing to maintain input/output level consistency

**Workaround**: Set `effects_enabled = false` in default channel configuration to bypass effects processing entirely.

**Files to Investigate**:
- `src-tauri/src/audio/mixer/stream_management/audio_input_stream.rs` (process_with_effects)
- `src-tauri/src/audio/types.rs` (AudioChannel default configuration)  
- `src-tauri/src/audio/effects/` (default effect settings)