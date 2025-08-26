# Application Audio Capture Design

## Overview

This document outlines the design for adding direct application audio capture (similar to Loopback) to Sendin Beats, allowing users to capture audio directly from applications like Spotify, iTunes, Tidal, etc. without requiring BlackHole configuration.

## Technical Foundation

### Core Technology: macOS Core Audio Taps (14.4+)

Apple introduced **Core Audio Taps** in macOS 14.4 that enables direct application audio capture:

- **API**: `AudioHardwareCreateProcessTap()` 
- **Permission Required**: `NSAudioCaptureUsageDescription` in Info.plist
- **Minimum macOS**: 14.4 (Sonoma)
- **No Virtual Devices Required**: Direct capture from application processes

### Key APIs Involved

```c
// Core APIs for application audio capture
OSStatus AudioHardwareCreateProcessTap(const CATapDescription *inDescription, AudioObjectID *outTapObjectID);
OSStatus AudioHardwareDestroyProcessTap(AudioObjectID inTapObjectID);
OSStatus AudioHardwareCreateAggregateDevice(const AudioAggregateDeviceDescription *inDescription, AudioObjectID *outDeviceObjectID);

// Process enumeration and translation
OSStatus AudioHardwareGetProperty(AudioObjectPropertyAddress *inAddress, UInt32 *ioDataSize, void *outData);
// Property: kAudioHardwarePropertyTranslatePIDToProcessObject
```

## Implementation Architecture

### 1. Process Discovery Layer

**Crate**: `sysinfo` for cross-platform process enumeration
**Purpose**: Discover running audio applications

```rust
pub struct ApplicationDiscovery {
    system: System,
    audio_apps: HashMap<String, ProcessInfo>,
}

pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub bundle_id: Option<String>,
    pub icon: Option<PathBuf>,
    pub is_audio_capable: bool,
}

impl ApplicationDiscovery {
    // Discover audio-capable applications
    pub fn scan_audio_applications(&mut self) -> Vec<ProcessInfo>;
    
    // Filter for common audio apps (Spotify, iTunes, etc.)
    pub fn get_known_audio_apps(&self) -> Vec<ProcessInfo>;
    
    // Check if app is currently playing audio
    pub fn is_app_playing_audio(&self, pid: u32) -> bool;
}
```

### 2. Core Audio Tap Interface

**Crate**: `coreaudio-sys` for raw bindings (may need custom bindings for new APIs)
**Purpose**: Interface with macOS Core Audio taps

```rust
pub struct ApplicationAudioTap {
    tap_id: AudioObjectID,
    aggregate_device_id: AudioObjectID,
    process_info: ProcessInfo,
    format: AudioStreamBasicDescription,
    is_capturing: bool,
}

impl ApplicationAudioTap {
    // Create tap for specific application
    pub async fn create_for_process(pid: u32) -> Result<Self, AudioTapError>;
    
    // Start capturing audio from application
    pub fn start_capture(&mut self) -> Result<(), AudioTapError>;
    
    // Get audio stream receiver
    pub fn get_audio_receiver(&self) -> broadcast::Receiver<Vec<f32>>;
    
    // Stop and cleanup
    pub fn stop_capture(&mut self) -> Result<(), AudioTapError>;
}
```

### 3. Application Audio Manager

**Purpose**: High-level management of multiple application taps

```rust
pub struct ApplicationAudioManager {
    active_taps: HashMap<u32, ApplicationAudioTap>, // PID -> Tap
    discovery: ApplicationDiscovery,
    permission_granted: bool,
}

impl ApplicationAudioManager {
    pub async fn new() -> Result<Self, AudioManagerError>;
    
    // Check and request audio capture permissions
    pub async fn request_permissions() -> Result<bool, AudioManagerError>;
    
    // Get list of capturable applications  
    pub fn get_available_applications(&mut self) -> Vec<ProcessInfo>;
    
    // Start capturing from specific app
    pub async fn start_capturing_app(&mut self, pid: u32) -> Result<broadcast::Receiver<Vec<f32>>, AudioManagerError>;
    
    // Stop capturing from app
    pub fn stop_capturing_app(&mut self, pid: u32) -> Result<(), AudioManagerError>;
    
    // Get currently active captures
    pub fn get_active_captures(&self) -> Vec<&ProcessInfo>;
}
```

### 4. UI Integration

**Frontend Components**: New application selector UI

```typescript
// New types for application audio sources
export type ApplicationAudioSource = {
  pid: number;
  name: string;
  bundleId?: string;
  icon?: string;
  isPlaying: boolean;
  isCapturing: boolean;
};

// New hook for application audio management
export const useApplicationAudio = () => {
  const [availableApps, setAvailableApps] = useState<ApplicationAudioSource[]>([]);
  const [activeCaptures, setActiveCaptures] = useState<ApplicationAudioSource[]>([]);
  
  const startCapturing = async (pid: number) => {
    // Start capturing from application
  };
  
  const stopCapturing = async (pid: number) => {
    // Stop capturing from application
  };
  
  return {
    availableApps,
    activeCaptures,
    startCapturing,
    stopCapturing,
    refreshApplications,
  };
};
```

## Implementation Plan

### Phase 1: Foundation (Week 1)
1. **Core Audio Bindings**
   - Investigate current `coreaudio-sys` support for `AudioHardwareCreateProcessTap`
   - Create custom bindings if necessary for macOS 14.4+ APIs
   - Test basic process tap creation and destruction

2. **Process Discovery**
   - Implement `ApplicationDiscovery` using `sysinfo` crate
   - Add filtering for known audio applications (Spotify, iTunes, etc.)
   - Test process enumeration and PID translation

### Phase 2: Audio Capture (Week 2)
1. **Tap Implementation**
   - Implement `ApplicationAudioTap` with basic capture functionality
   - Test audio stream capture from target applications
   - Verify audio format handling and conversion

2. **Permission Handling**
   - Add `NSAudioCaptureUsageDescription` to Info.plist
   - Implement permission checking and requesting
   - Handle permission denied scenarios gracefully

### Phase 3: Integration (Week 3)
1. **Manager Layer**
   - Implement `ApplicationAudioManager`
   - Add support for multiple simultaneous captures
   - Integrate with existing audio mixer pipeline

2. **Tauri Commands**
   - Add commands for application discovery and capture control
   - Integrate with existing audio device management
   - Add error handling and state management

### Phase 4: UI (Week 4)
1. **Frontend Components**
   - Create application selector UI components
   - Add application icons and status indicators
   - Integrate with existing mixer channel system

2. **User Experience**
   - Add application audio sources as mixer input options
   - Display capture status and controls
   - Handle application lifecycle (app quit, app launch)

## Technical Challenges and Solutions

### 1. macOS Version Compatibility
**Challenge**: Core Audio Taps only available on macOS 14.4+
**Solution**: 
- Runtime version detection
- Graceful fallback to "Use BlackHole" instructions for older systems
- Feature flag for tap-based capture

### 2. Permission Management
**Challenge**: Audio capture requires user permission
**Solution**:
- Clear permission dialogs with explanations
- Graceful handling of permission denial
- Instructions for enabling permissions in System Preferences

### 3. Application Lifecycle
**Challenge**: Applications can quit/launch during capture
**Solution**:
- Monitor process lifecycle
- Automatic cleanup of dead taps
- Reconnection logic for application restarts

### 4. Audio Format Compatibility
**Challenge**: Different applications may output different formats
**Solution**:
- Format detection and conversion
- Standardize to mixer's internal format (48kHz, stereo, f32)
- Handle sample rate and channel count mismatches

### 5. Performance Impact
**Challenge**: Multiple application taps could impact performance
**Solution**:
- Limit maximum simultaneous captures (e.g., 4 applications)
- Efficient audio buffer management
- Background thread processing

## Error Handling

### Common Error Scenarios
1. **Permission Denied**: Guide user to System Preferences
2. **Application Not Found**: Handle PID changes and app quits
3. **Audio Format Unsupported**: Fallback or conversion
4. **System Resources**: Limit concurrent captures
5. **macOS Version Too Old**: Fallback to BlackHole instructions

### Error Types
```rust
#[derive(Debug, thiserror::Error)]
pub enum ApplicationAudioError {
    #[error("Permission denied - audio capture not authorized")]
    PermissionDenied,
    
    #[error("Application not found (PID: {pid})")]
    ApplicationNotFound { pid: u32 },
    
    #[error("Core Audio error: {0}")]
    CoreAudioError(OSStatus),
    
    #[error("Unsupported macOS version - requires 14.4+")]
    UnsupportedSystem,
    
    #[error("Too many active captures (max: {max})")]
    TooManyCaptures { max: usize },
}
```

## Testing Strategy

### Unit Tests
- Process discovery and filtering
- Audio format conversion
- Permission state management

### Integration Tests  
- End-to-end application capture
- Multiple simultaneous captures
- Error scenario handling

### Manual Testing
- Test with Spotify, iTunes, Tidal, YouTube
- Test permission dialogs and denied states
- Test application lifecycle scenarios
- Performance testing with multiple captures

## Future Enhancements

### System Audio Capture
- Capture all system audio (not just specific apps)
- System audio + application mixing
- Exclusion filters (capture system except specific apps)

### Advanced Features
- Per-application volume control
- Application-specific effects chains
- Audio routing matrix (app A to channel 1, app B to channel 2)
- Application grouping and batch control

### Cross-Platform Support
- Windows: WASAPI loopback capture
- Linux: PulseAudio module-loopback equivalent
- Unified API across platforms

## Conclusion

This design provides a comprehensive approach to adding direct application audio capture to Sendin Beats, matching Loopback's functionality while integrating seamlessly with our existing mixer architecture. The phased implementation approach allows for incremental development and testing, while the fallback strategies ensure compatibility across different macOS versions.

The key innovation is leveraging Apple's new Core Audio Taps API (macOS 14.4+) to eliminate the need for complex BlackHole configurations, providing users with a seamless "just works" experience for capturing audio from their favorite applications.