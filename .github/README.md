# GitHub Actions CI/CD

This directory contains GitHub Actions workflows for continuous integration and testing.

## Workflows

### 🚀 `test.yml` - Full Test Suite
**Triggers**: Push/PR to `main` or `develop` branches

**Jobs**:
- **rust-tests**: Runs on Ubuntu, macOS, and Windows
  - Formatting check (`cargo fmt`)
  - Linting (`cargo clippy`) 
  - Build verification
  - All Rust unit and integration tests
  
- **frontend-tests**: TypeScript compilation and checks
  - Type checking with `tsc`
  - Build verification
  
- **build-test**: Full Tauri application build test
  - Cross-platform build verification (no bundle)
  - Ensures app builds successfully on all platforms
  
- **audio-tests**: macOS-only audio system tests
  - CoreAudio-specific tests
  - Audio device enumeration tests

### ⚡ `quick-check.yml` - Fast Feedback
**Triggers**: Push to any branch, PR to `main`/`develop`

**Purpose**: Provides fast feedback on basic compilation and core tests
- Quick Rust compilation check (`cargo check`)
- TypeScript build
- Library-only tests (fastest subset)

## Local Development Scripts

Run these commands locally to match CI behavior:

```bash
# Run all tests
pnpm test

# Check compilation without running tests  
pnpm check

# Format code
pnpm fmt

# Run linter
pnpm clippy

# Audio-specific tests
pnpm test:audio
```

## Test Structure

**Rust Tests**: Located in `src-tauri/tests/`
- Audio device detection and enumeration
- Effects processing 
- Stream management
- Configuration validation
- CoreAudio integration (macOS)

**Current Status**: 
- ✅ 6 passing tests
- ⚠️ 1 failing test (device enumeration - platform-specific)

## Platform-Specific Notes

- **Ubuntu**: Requires ALSA, PulseAudio, and GTK dev libraries
- **macOS**: Uses CoreAudio framework (native)
- **Windows**: Uses WASAPI (native)

Audio tests may behave differently on CI runners due to virtual/headless environments.