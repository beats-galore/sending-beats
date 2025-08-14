# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Vision

Sendin Beats is a comprehensive, multi-phased radio streaming platform designed to be a fully-fledged application for DJs to livestream to Icecast internet radio streaming providers. The project aims to replace and enhance functionality found in tools like Ladiocast/Loopback and radio.co, providing an all-in-one solution for professional radio streaming.

## Current Implementation Status

**Phase**: Early development - Basic streaming client with foundational UI components
**Architecture**: Tauri (Rust backend) + React TypeScript frontend
**Current Features**: Basic DJ streaming interface, listener player, admin panel mockups

## Development Phases (Revised Priority Order)

### Phase 1: Core Local Audio Functionality
**Focus**: Get essential audio mixing and streaming working on local machine
- [ ] Virtual audio mixer implementation (Ladiocast/Loopback equivalent)
- [ ] Multi-channel input support (mic, system audio, applications, audio interfaces)
- [ ] Multi-channel output routing (headphones, speakers, stream output)
- [ ] Real-time audio processing and mixing
- [ ] Icecast connection and streaming (non-persistent configuration initially)
- [ ] DJ controller integration
- [ ] Audio device management and routing

### Phase 2: Enhanced UI/UX & Local Persistence
**Focus**: Professional interface and local configuration storage
- [ ] Modern, sleek UI design system overhaul
- [ ] Local storage for stream configurations and mixer settings
- [ ] Advanced audio controls and visualization
- [ ] Recording and playback functionality
- [ ] Preset management for different streaming scenarios

### Phase 3: Multi-User & Cloud Integration
**Focus**: Supabase integration and multi-organization features
- [ ] User authentication and authorization
- [ ] Multi-organization, multi-user support with permissioning
- [ ] Cloud-synced configurations and settings
- [ ] Schedule management system (radio.co replacement)
- [ ] Fallback content automation and management

## Core Application Features (Target)

### Virtual Audio Mixer (Priority 1)
- **Multi-Channel Input Support**:
  - Microphone inputs
  - Application audio capture (Spotify, iTunes, system audio)
  - Audio interfaces and DJ controllers
  - Virtual audio devices (BlackHole 2CH equivalent)
  
- **Multi-Channel Output Routing**:
  - Headphone monitoring
  - System speakers
  - Main output to Icecast stream
  - Cue/preview channels

### Stream Management (Priority 1)
- **Basic Connection**: Direct Icecast streaming with configurable settings
- **Audio Quality**: Multiple bitrate and encoding options
- **Real-time Monitoring**: Connection status, listener counts, audio levels

### Future Features (Phase 3)
- **Schedule System**: Time-based programming with user permissions
- **Fallback Behavior**: Automated content during off-hours
- **Multi-Organization**: Role-based access control across organizations

## Development Commands

```bash
# Start development server (both frontend and backend)
pnpm dev

# Build the application
pnpm build

# Type checking
tsc

# Preview built application
pnpm preview

# Tauri-specific commands
pnpm tauri dev      # Development with hot reload
pnpm tauri build    # Production build
```

## Current Architecture

### Frontend (React + TypeScript)
- **App.tsx**: Main application with view routing
- **DJClient.tsx**: Basic streaming interface (to be enhanced with virtual mixer)
- **ListenerPlayer.tsx**: Audio playback interface
- **AdminPanel.tsx**: Management dashboard (will be simplified for Phase 1)

### Backend (Rust via Tauri)
- **Streaming Engine**: Core Icecast integration
- **Audio Processing**: LAME encoding, real-time audio handling
- **API Layer**: Tauri commands for frontend-backend communication

### Current Dependencies
```json
// Audio Processing (to be expanded)
"lamejs": "^1.2.1",           // MP3 encoding
"opus-recorder": "^8.0.5",    // Audio recording
"hls.js": "^1.5.0",          // Streaming support

// UI Framework
"@headlessui/react": "^1.7.0", // UI components
"@heroicons/react": "^2.0.0",  // Icons
"tailwindcss": "^3.4.0",      // Styling (needs design overhaul)

// State Management
"zustand": "^4.4.0",          // React state
"axios": "^1.6.0",           // HTTP client
"socket.io-client": "^4.7.0" // Real-time communication
```

## Phase 1 Development Priorities

### Core Audio System
1. **Audio Device Enumeration**: Detect all available input/output devices
2. **Virtual Mixer Engine**: Real-time audio mixing with multiple channels
3. **Input Processing**: Handle microphone, system audio, and application audio
4. **Output Routing**: Route mixed audio to stream and monitoring outputs
5. **Audio Quality Control**: Gain, EQ, compression per channel

### Streaming Integration
1. **Icecast Connection**: Robust connection handling with reconnection logic
2. **Audio Encoding**: High-quality MP3/OGG encoding for streaming
3. **Metadata Management**: Song title, artist, show information
4. **Connection Monitoring**: Real-time status and listener statistics

### UI/UX for Phase 1
1. **Professional Audio Interface**: Channel strips, faders, VU meters
2. **Device Selection**: Easy audio device routing interface
3. **Stream Configuration**: Simple, clear connection setup
4. **Real-time Feedback**: Visual audio levels, connection status, stream health

## Technical Considerations for Phase 1

### Audio Engineering Requirements
- Ultra-low latency audio processing (< 10ms)
- Cross-platform audio subsystem integration
- Real-time mixing with multiple simultaneous inputs
- Professional-grade audio quality maintenance

### Cross-Platform Audio Challenges
- **macOS**: Core Audio integration, potential BlackHole integration
- **Windows**: WASAPI/DirectSound integration, virtual audio cables
- **Linux**: ALSA/PulseAudio integration, JACK compatibility

### Performance Requirements
- Real-time audio processing without dropouts
- Efficient CPU usage for sustained streaming
- Memory management for continuous operation
- Thread safety for audio and UI operations

## Testing Strategy for Phase 1
- Audio latency testing across different buffer sizes
- Multi-input stress testing (simultaneous sources)
- Stream stability testing with various Icecast servers
- Cross-platform audio device compatibility
- Long-running stability tests