# Sweet Beats Studio

A radio streaming application for DJs. Live audio mixing, recording, and Icecast
broadcasting in one desktop app — the job Ladiocast and Loopback do, without the
hand-off between them.

Tauri (Rust) with a React TypeScript frontend. **macOS only** for now: the audio
path is built directly on Core Audio and ScreenCaptureKit.

## What works

**Mixing**

- Multiple live inputs mixed to multiple destinations, at whatever sample rates
  the hardware runs at
- Bus routing — an input can reach some destinations and not others, so a source
  can be monitored without going out on the broadcast
- Per-channel gain, pan, mute and solo
- Peak/RMS metering per channel, per bus, and on the master
- Latency accounting for every stage of the pipeline

**Sources**

- Core Audio input devices, including virtual ones such as BlackHole
- Per-application audio capture via ScreenCaptureKit
- System audio
- File playback

**Destinations**

- Any Core Audio output device
- Icecast broadcast
- Recording to WAV (16, 24 or 32-bit PCM) or MP3

Recording and Icecast register as ordinary mixer outputs, so bus routing applies
to them the same way it applies to hardware.

**Devices**

- Hotplug detection, so a device appearing or disappearing is noticed without a
  refresh
- Health tracking with recovery for devices that fail while still listed
- A reconnected input returns to the channel it was patched to

**State**

- SQLite, with mixer configurations, channel names, device routing and patch
  colours stored per session

## Not working yet

Worth knowing before you go looking for these:

- **EQ, compressor and limiter do not affect audio.** The controls and the DSP
  both exist, but the chain is not wired into the signal path — only gain, pan,
  mute and solo reach the audio.
- **FLAC recording** falls back to WAV.
- **Windows and Linux** are not supported. There is no non-Core Audio
  implementation of the capture or output path.

Anything else that is missing or broken is tracked in
[issues](https://github.com/beats-galore/sweet-beats-studio/issues).

## Getting started

Requirements: [Rust](https://rustup.rs/) (stable), Node 18+,
[pnpm](https://pnpm.io/), Xcode command line tools, and macOS.

```bash
pnpm install
pnpm tauri:dev
```

`tauri:dev` builds the Swift ScreenCaptureKit helper first, then starts the app
with logs written to `logs/output.log`.

macOS will ask for microphone and screen recording permission the first time
audio is captured. Screen recording is what ScreenCaptureKit needs to capture
another application's audio.

## Working on it

```bash
turbo rust:check          # type check the backend
pnpm test                 # backend tests
turbo lint:fix -- <paths> # lint changed frontend files
turbo rust:fmt            # format Rust
```

Run commands from the repository root. `turbo` drives both sides of the project.

### Database

Migrations live in `src-tauri/migrations` and run automatically on startup.

```bash
pnpm migration <name>   # create a migration
pnpm migrate            # apply pending migrations, needs cargo install sqlx-cli
```

Starting the app is usually enough — `pnpm migrate` only exists for applying
them without a launch, and needs `sqlx-cli` installed separately.

Schema changes need matching SeaORM entities in `src-tauri/src/entities`.

## How it is put together

Audio runs as a four-layer pipeline, each layer on its own thread and connected
by lock-free ring buffers:

1. **Capture** — Core Audio and ScreenCaptureKit callbacks write incoming audio
2. **Input workers** — resample to the mix rate, apply channel controls, meter
3. **Mixing** — sum each bus and hand it to the outputs taking it
4. **Output workers** — resample to each device's rate and write to hardware

The mixing layer produces on a fixed block, paced by how much the outputs are
holding, so the output hardware's drain rate is what clocks the mixer.

```
src-tauri/src/
├── audio/
│   ├── mixer/pipeline/     the four layers, bus routing, block accumulation
│   ├── devices/            enumeration, Core Audio streams, hotplug, health
│   ├── effects/            EQ, compressor, limiter, channel controls
│   ├── recording/          encoders and file writing
│   ├── broadcasting/       Icecast
│   ├── screencapture/      per-application capture (Swift FFI)
│   └── file_player/
├── commands/               Tauri command surface
├── db/                     services over the SQLite schema
└── entities/               SeaORM models
src-swift/                  ScreenCaptureKit helper
src/                        React frontend
```

## License

Not yet licensed.
