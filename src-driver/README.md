# Sendin Beats Virtual Audio Driver

A CoreAudio AudioServerPlugin that creates a virtual loopback audio device for system audio capture without echo/doubling issues.

## Purpose

Solves the fundamental problem with system audio mixing:
- System audio normally plays directly to physical outputs
- Capturing it with ScreenCaptureKit while it plays creates double audio (direct + delayed through mixer)
- This virtual device intercepts system audio SILENTLY, allowing clean capture for mixing

## Architecture

```
System Audio → Virtual Device (silent, just ring buffer)
                     ↓
              Ring buffer stores it
                     ↓
         App captures from input side
                     ↓
            Mix with mic/other inputs
                     ↓
        Output mixed result to REAL speakers
```

**Result**: System audio only plays ONCE (after mixing), not twice.

## How It Works

Based on BlackHole's ring buffer architecture:
- **Output Stream**: System writes audio → ring buffer (no physical playback)
- **Input Stream**: App reads audio ← ring buffer
- 2-channel stereo, 32-bit float
- Supports sample rates: 8kHz - 192kHz

## Building

```bash
cd src-driver
make
```

Produces: `build/SendinBeatsAudio.driver`

## Manual Installation (for testing)

```bash
cd src-driver
make install
```

This will:
1. Copy driver to `/Library/Audio/Plug-Ins/HAL/`
2. Restart `coreaudiod` to load the driver

**Note**: Requires `sudo` and will prompt for password.

## Automatic Installation (production)

The driver is bundled in the Tauri app at `Resources/driver/SendinBeatsAudio.driver`.

On first run, `VirtualDriverManager::install()` will:
1. Check if driver is already installed
2. If not, copy from bundle to `/Library/Audio/Plug-Ins/HAL/`
3. Restart `coreaudiod`
4. Verify installation

## Usage in Code

```rust
use crate::audio::devices::VirtualDriverManager;

// Check if installed
if !VirtualDriverManager::is_installed() {
    VirtualDriverManager::install().await?;
}

// Get device UID for CoreAudio operations
let device_uid = VirtualDriverManager::get_device_uid(); // "SendinBeatsAudio_UID"

// Set as system default (to capture system audio)
system_audio_router.divert_system_audio_to_virtual_device().await?;

// Now capture from the virtual device's input stream
// System audio goes to virtual device (silent) → we capture it → mix → output to real speakers
```

## Device Information

- **Name**: Sendin Beats Audio
- **UID**: `SendinBeatsAudio_UID`
- **Bundle ID**: `com.sendinbeats.audio.driver`
- **Channels**: 2 (stereo)
- **Format**: 32-bit float PCM

## Uninstallation

```bash
cd src-driver
make uninstall
```

Or via code:
```rust
VirtualDriverManager::uninstall().await?;
```

## Troubleshooting

**Driver not appearing in Audio MIDI Setup:**
```bash
sudo launchctl kickstart -kp system/com.apple.audio.coreaudiod
```

**Permission denied:**
- Driver installation requires sudo privileges
- User will be prompted for password

**Driver crashes:**
- Check Console.app for coreaudiod logs
- Look for errors in System log

## Credits

Based on [BlackHole](https://github.com/ExistentialAudio/BlackHole) by Existential Audio Inc.
- Open source AudioServerPlugin implementation
- Zero-latency ring buffer architecture
- Production-tested across thousands of users

## License

See parent project LICENSE file.
