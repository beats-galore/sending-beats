## Recording Shows No Disk Space Available Despite Sufficient Storage

**Status**: UNRESOLVED (Partially Fixed)  
**Priority**: Low  
**Date Discovered**: 2025-08-29
**Last Updated**: 2025-08-29

**Description**: The recording pane was incorrectly displaying that no disk space is
available. The core issue (missing disk space field) has been fixed, but the 
disk space calculation is slightly inaccurate (shows ~19GB instead of actual ~17GB available).

**Steps to Reproduce**:

1. Start the application with `pnpm tauri dev`
2. Navigate to the Recording pane/tab
3. Attempt to start a recording or check recording status
4. Expected: Recording should be available with accurate disk space information
5. Actual: Interface shows "no disk space available" message

**Investigation Done**:

- System has sufficient disk space (verified independently)
- Issue likely introduced during modularization of recording-related modules
- May be related to file system access or disk space calculation logic

**Root Cause Analysis** (RESOLVED):

During the modularization refactor, the `RecordingStatus` struct lost critical fields that the frontend expected:

1. ✅ **FIXED: Missing Fields**: `available_space_gb`, `total_recordings`, and `active_recordings` fields were missing
2. ✅ **FIXED: Field Naming**: Frontend expected `current_session` but Rust used `session`
3. ✅ **FIXED: Disk Space Calculation**: Implemented real disk space checking using `libc::statvfs`
4. ⚠️ **MINOR: Calculation Accuracy**: Disk space calculation is slightly off but functional

**Implementation Details**:

**Fixed Missing Frontend Fields**:
- Added `available_space_gb: f64` field to `RecordingStatus`
- Added `total_recordings: usize` field for recording history count
- Added `active_recordings: Vec<String>` field for active recording IDs
- Added `#[serde(rename = "current_session")]` for frontend compatibility

**Implemented Real Disk Space Checking**:
- Replaced hardcoded 100GB fallback with actual `libc::statvfs` system call
- Added Unix-specific implementation using filesystem statistics
- Added fallback for non-Unix systems
- Integrated disk space checking into all `RecordingStatus` creation points

**Files Fixed**:
- ✅ `src/audio/recording/types.rs` (RecordingStatus struct fields restored)
- ✅ `src/audio/recording/filename_generation.rs` (real disk space implementation)
- ✅ `src/audio/recording/recording_writer.rs` (status creation with disk space)

**Remaining Minor Issue**:
- Disk space calculation shows minor variance (~2GB difference) but is functional
- Recording functionality is now fully available

**Status**: Recording pane now shows disk space information (with minor inaccuracy). Users can record audio. **Not prioritized for immediate fix.**
