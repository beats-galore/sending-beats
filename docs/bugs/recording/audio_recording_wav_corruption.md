## Audio Recording Issues

### WAV File Compatibility Issue

**Status**: UNRESOLVED  
**Priority**: Medium  
**Date Discovered**: 2025-08-26

**Description**: WAV files recorded by the application can only be opened in VLC
player, but fail to open in iTunes, QuickTime, and other standard audio players
on macOS.

**Investigation Done**:

- Changed default bit depth from 24-bit to 16-bit (most compatible format)
- Updated both backend (`src-tauri/src/recording_service.rs:101`) and frontend
  (`src/components/dj/RecordingControlsCard.tsx:192`) defaults
- WAV encoder uses standard PCM format (format code 1) with correct header
  structure

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

**Workaround**: Users can convert files using VLC or other audio converters, or
use MP3 format instead.
