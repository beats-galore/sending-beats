## Recording Interface Overhaul

**Status**: PLANNED  
**Priority**: High  
**Date Identified**: 2025-08-30

**Description**: The current recording system needs a comprehensive overhaul to handle file management, metadata, and user experience properly. The interface lacks proper temporary file handling, complete metadata support, and actual metadata writing to output files.

**Current Issues**:

- No temporary file management - recordings write directly to final destination, risking corruption on crashes
- Incomplete metadata capture - missing essential audio file metadata fields
- Metadata not actually written to output file headers - current metadata is purely cosmetic
- No metadata presets or templates for common recording scenarios
- Poor user experience for metadata entry during recording workflow
- Risk of losing recordings if application crashes mid-recording

**Proposed Solution**:

- Implement temporary file recording with atomic move on completion
- Create comprehensive metadata input system with all standard audio file fields
- Integrate proper metadata writing into audio encoding pipeline
- Add metadata presets and templates for quick setup
- Implement recording state recovery for crash scenarios
- Design intuitive UI flow for metadata entry and validation

**Files Affected**:

- `src-tauri/src/audio/recording/recording_service.rs` - Core recording logic overhaul
- `src-tauri/src/audio/recording/recording_writer.rs` - Add temporary file handling
- `src-tauri/src/audio/recording/encoders.rs` - Integrate metadata writing into encoding
- `src-tauri/src/audio/recording/types.rs` - Expand metadata types
- `src-tauri/src/commands/recording.rs` - Update recording commands for new metadata
- `src/components/dj/RecordingConfigCard.tsx` - Complete UI overhaul
- `src/types/audio.types.ts` - Add comprehensive metadata types
- `src/hooks/use-recording.ts` - Update recording state management

**Implementation Steps**:

1. **Temporary File System** - Implement recording to `.tmp` files with atomic move on completion
2. **Comprehensive Metadata Types** - Define complete metadata structure matching audio file standards
3. **Metadata Encoding Integration** - Wire metadata into MP3/FLAC/WAV encoding pipeline
4. **UI Components** - Build comprehensive metadata input forms with presets
5. **State Recovery** - Add crash recovery for in-progress recordings
6. **User Experience Flow** - Design intuitive recording workflow with metadata validation

**Metadata Fields to Implement**:

- **Core Fields**: Artist Name, Track Title, Album Title, Track Number, Year, Genre
- **Extended Fields**: Album Artist, Composer, Comments, Copyright, BPM
- **Technical Fields**: Encoder, Encoding Date, Sample Rate, Bitrate
- **Artwork**: Album cover image embedding
- **Custom Fields**: User-defined tags and categories

**Testing Strategy**:

- Test temporary file handling with application crashes during recording
- Verify metadata is correctly written to all supported audio formats (MP3, FLAC, WAV)
- Test metadata presets and template functionality
- Validate file corruption scenarios and recovery mechanisms
- Test UI workflow with various metadata input combinations
- Performance testing for long recordings with metadata

**Breaking Changes**: 
- Recording configuration API will require metadata fields
- Recording file structure changes (temporary files)
- Frontend recording interface complete redesign

**Estimated Effort**: 2-3 weeks - This is a substantial refactor touching multiple layers of the application