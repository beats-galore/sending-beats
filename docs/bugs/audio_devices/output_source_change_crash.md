### Output Source Change Crashes Application

**Status**: UNRESOLVED  
**Priority**: High  
**Date Discovered**: 2025-08-26

**Description**: Changing the audio output source after it has been initially
selected causes the entire application to crash.

**Investigation Done**:

- Issue reported by user during testing
- This is a regression or persistent issue that affects core audio functionality

**Next Steps**:

1. Check audio device switching logic in backend
2. Look for resource cleanup issues when switching output devices
3. Test with different output devices (speakers, headphones, virtual devices)
4. Add proper error handling and device switching safety measures
5. Check if this is related to previous audio device management fixes

**Workaround**: Set correct output device before starting audio, avoid changing
after initialization.
