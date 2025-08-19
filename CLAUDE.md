# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Vision

Sendin Beats is a comprehensive, multi-phased radio streaming platform designed to be a fully-fledged application for DJs to livestream to Icecast internet radio streaming providers. The project aims to replace and enhance functionality found in tools like Ladiocast/Loopback and radio.co, providing an all-in-one solution for professional radio streaming.

## Implementation guidance
- Approach things in small chunks. Work iteratively towards a simple goal before spreading out into different feature areas.
- Find logical breaking points when working, and commit before the scope of changes is before long with a detailed description of the work done so far. Make sure you check the diff of code before committing.
- Don't be afraid to ask questions about suggested solutions. You don't necessarily need to work completely isolated until the goal is achieved. It's good to ask for feedback


## Current Implementation Status

**Phase**: Early development - Virtual mixer UI implementation with backend infrastructure
**Architecture**: Tauri (Rust backend) + React TypeScript frontend
**Current Features**: Professional virtual mixer interface, audio device enumeration, streaming client foundation

## URGENT ISSUES TO FIX (Next Session Priority)

### Critical Audio Issues
1. **NO ACTUAL AUDIO CAPTURE**: Currently only generating test/animated levels - need to implement real audio input capture from selected devices
2. **NO ACTUAL AUDIO OUTPUT**: No sound playing through selected output devices (speakers/headphones) - need cpal output stream implementation
3. **NO REAL AUDIO PROCESSING**: VU meters show test animation instead of actual audio levels from captured audio
4. **HORIZONTAL LAYOUT**: UI successfully converted to horizontal layout as requested, but needs audio functionality

### What's Currently Working
- ✅ Virtual mixer UI with horizontal layout
- ✅ Professional channel strips with gain, pan, EQ, compressor, limiter controls
- ✅ Master section with output device selection and master VU meters
- ✅ Audio device enumeration (detects devices correctly)
- ✅ VU meter components with animated test levels
- ✅ Backend generates test audio levels and frontend displays them
- ✅ Tauri commands for mixer control and real-time data polling

### What Needs Immediate Attention
- 🚨 **Audio Input Streams**: Implement cpal input stream creation for selected input devices
- 🚨 **Audio Output Streams**: Implement cpal output stream for master audio output to speakers
- 🚨 **Real Audio Processing**: Replace test level generation with actual audio level calculation from captured samples
- 🚨 **Audio Threading**: Proper audio processing thread with input → effects → output chain
- 🚨 **Buffer Management**: Implement proper audio buffer management for low-latency processing

## Recent Progress (Previous Session)

### Successfully Implemented
1. **Full Virtual Mixer Interface**: Professional mixing console with:
   - Channel strips in horizontal layout (as requested by user)
   - Gain, pan, input device selection, mute/solo controls per channel
   - 3-band EQ (high/mid/low) with ±12dB range
   - Compressor with threshold, ratio, attack, release controls
   - Limiter with threshold control
   - Real-time VU meters (currently test data, need real audio)

2. **Master Section**: 
   - Master output device selection
   - Master gain control
   - Stereo master VU meters (L/R)
   - Audio metrics display (CPU, sample rate, latency, channels)

3. **Backend Infrastructure**:
   - Comprehensive audio device enumeration with device filtering
   - Real-time VU meter data polling (100ms intervals)
   - Professional audio effects chain structures (EQ, compressor, limiter)
   - Tauri commands for all mixer operations

4. **UI/UX Improvements**:
   - Converted from vertical to horizontal layout as requested
   - Professional color coding for VU meters (green/yellow/red)
   - Responsive grid layout that adapts to screen size
   - Clean, professional mixer aesthetic

### User Feedback Addressed
- ✅ Fixed horizontal layout issue (was reverted to vertical, now corrected)
- ⚠️ **Still need to fix**: No actual audio capture or output (critical issue)
- ⚠️ **Still need to fix**: VU meters show test animation instead of real levels

## Development Commands

```bash
# Start development server (CORRECT COMMAND - user specified)
pnpm tauri dev

# NOTE: User specifically said "Don't ever use npm unless it's installing global dependencies"
# Always use pnpm for this project

# Type checking
tsc

# Build for production
pnpm tauri build
```

## Architecture Status

### Frontend (React + TypeScript)
- **App.tsx**: Routes to Virtual Mixer as default (changed from 'home' to 'mixer')
- **VirtualMixer.tsx**: Complete professional mixer interface
  - ChannelStrip component with full controls
  - VUMeter component with professional visualization
  - Master section with output routing
  - Real-time polling for VU meter updates
- **Tauri Window**: Configured for full-screen (maximized: true, 1400x1000)

### Backend (Rust via Tauri)
- **audio.rs**: 
  - AudioDeviceManager with device enumeration and filtering
  - VirtualMixer structure with effects chains
  - Currently generates test levels - NEEDS REAL AUDIO IMPLEMENTATION
  - Professional audio effects structures (ThreeBandEqualizer, Compressor, Limiter)
- **lib.rs**: All Tauri commands implemented for mixer control

### Critical Code Locations
- **VU Meter Implementation**: `/src/components/VirtualMixer.tsx:65-121`
- **Channel Strip Layout**: `/src/components/VirtualMixer.tsx:123-200+`
- **Audio Backend**: `/src-tauri/src/audio.rs` (needs input/output stream implementation)
- **Test Level Generation**: `/src-tauri/src/audio.rs:~800+` (replace with real audio capture)

## Next Session Goals

### Phase 1: Get Real Audio Working
1. **Fix Audio Input Capture**:
   - Implement cpal input stream creation in VirtualMixer::add_input_stream()
   - Connect selected input devices to actual audio capture
   - Process captured samples through audio buffer system

2. **Fix Audio Output**:
   - Implement cpal output stream in VirtualMixer::set_output_stream()
   - Route mixed audio to selected output device (speakers/headphones)
   - Ensure audio flows from input → processing → output

3. **Fix VU Meter Levels**:
   - Replace test animation with real audio level calculation
   - Calculate peak/RMS from captured audio samples
   - Update VU meters with actual audio data instead of sine wave test

4. **Audio Processing Chain**:
   - Connect input audio through effects (EQ, compressor, limiter)
   - Apply channel settings (gain, pan, mute, solo) to real audio
   - Mix multiple channels to master output

### Success Criteria for Next Session
- [ ] User can select input device and hear their microphone/system audio
- [ ] User can select output device and hear audio through speakers/headphones
- [ ] VU meters respond to actual audio levels, not test animation
- [ ] Audio flows: Input Device → Channel Processing → Master Mix → Output Device
- [ ] Channel controls (gain, pan, EQ) affect the actual audio output

### Technical Implementation Notes
- User has BlackHole 2CH, microphone, MacBook speakers, and BenQ monitor available
- Focus on macOS Core Audio implementation first
- Use cpal for cross-platform audio stream management
- Audio processing should happen in separate thread from UI
- Maintain horizontal layout that user requested

## Known Working Components
- Device enumeration and filtering works correctly
- UI polling and updates work at 10 FPS (100ms intervals)
- Professional mixer interface is complete and responsive
- Backend-frontend communication is solid
- Effects parameter structures are implemented