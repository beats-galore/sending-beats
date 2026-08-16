# Events vs. Polling (Frontend ↔ Backend)

**TL;DR — yes, Tauri can push, and this codebase already does it three ways.**
Device changes and now-playing use `app.emit` events; VU meters use
`tauri::ipc::Channel` (a streamed IPC channel, the right tool for high-rate
data). No webhooks or extra transport needed. What remains are **six polling
loops** that re-fetch state the backend already knows changed — every one of
them is convertible to push with existing infrastructure, and one of them
(latency) duplicates a sampling loop the backend already runs.

## The two push mechanisms (already in use)

| Mechanism | Best for | Where it's used today |
|---|---|---|
| `app.emit(event, payload)` + `listen()` | Low-rate state changes | `devices/device_watcher.rs:507` (`DEVICES_CHANGED`, `DEVICE_DISCONNECTED`, `DEVICE_RECONNECTED`), `now_playing/watcher.rs:454` (`NOW_PLAYING_CHANGED`, `NOW_PLAYING_ERROR`) |
| `tauri::ipc::Channel<T>` | High-rate streams | VU meters (`use-vu-channel-stream.ts:99` ↔ `vu_channel_service.rs`) |

Rule of thumb: `emit` for things that change on the order of seconds; `Channel`
for things that change on the order of frames.

## The polling loops, and what each should become

### 1. `use-pipeline-latency.ts` → `get_pipeline_latency` — *pure duplication*

The backend **already samples latency on a 500 ms window**: the reporter task
spawned in `pipeline_manager.rs:568-572` closes a gauge window every
`LATENCY_SAMPLE_INTERVAL` and aggregates a `LatencySnapshot`. The frontend then
independently polls the same probe on its own timer. Emit the snapshot at the
end of each reporter window (`latency:snapshot` event) and delete the poll.
This is the cleanest conversion in the codebase — the producer-side cadence
already exists.

### 2. `use-recording.ts` → `get_recording_status`

Recording state changes on explicit transitions (start, stop, auto-stop,
split) — all known to `RecordingService` at the moment they happen. Emit
`recording:status` on transitions; derive elapsed time client-side between
events (the UI already knows the start timestamp). Polling only exists to
notice transitions late.

### 3. `use-streaming-status.ts` → `get_icecast_streaming_status`

Same shape: connection state is a state machine inside the streaming service;
listener/bitrate stats change slowly. Emit on state change + a periodic stats
event from the service's own monitor task (one already exists —
`manager.rs`'s monitor — it just doesn't emit).

### 4. `use-listener-stats.ts` — Icecast server stats (remote HTTP)

Polling is inherent here (the data lives on the Icecast server), but it's in
the wrong place: each webview polls independently. Move the poll into the
backend (one poller, owned by the streaming service), emit `cast:listeners`
deltas. Bonus: survives webview reloads, dedupes across windows, and the
credential handling stays server-side.

### 5. `use-file-player.ts` → `FilePlayerStore.poll()` — *the events already half-exist*

`PlayerEvent` (`file_player/player.rs:104`) already carries `TrackFinished`
internally (used for auto-advance via `set_event_sender`). It never reaches the
frontend. Emit `player:state` on play/pause/seek/track-change/queue-change and
interpolate the playhead client-side — which the code already does elsewhere
(`use-track-position.ts` ticks a local interpolation). With push + local
interpolation, the poll deletes entirely.

### 6. `use-process-metrics.ts`

Fine to poll (it's a debug surface), or emit on a backend interval. Lowest
priority.

## Why this matters beyond elegance

- **Latency of visibility**: a poll at interval N averages N/2 of staleness on
  every state change; push is immediate. For transport controls (record armed,
  on-air) staleness is user-visible.
- **Wasted IPC + wakeups**: six independent timers × invoke round-trips × JSON
  serialization for data that mostly hasn't changed. The audio coordinator
  serializes some of these command handlers behind device operations (doc 03
  §5), so polls can also *stall* — and then burst.
- **Webview reloads**: the VU channel path already solved re-registration after
  reload (`SharedVUChannel` re-read per batch — `vu_channel_service.rs:19`);
  event listeners get this for free, per-hook polls each re-implement lifecycle.

## Suggested conventions when converting

1. **One typed event registry.** A single `events.rs` with `pub const` event
   names + payload structs (serde), mirrored by one TS file with the payload
   types. The device watcher and now-playing modules each invented their own
   convention; a third convention per future feature is the default outcome
   otherwise.
2. **Emit state, not deltas, on low-rate events** (idempotent re-render;
   listeners that connect late get a full picture on the next change; pair
   with one `get_*` command for initial hydration — the existing commands
   already serve this).
3. **Keep `Channel` for ≥10 Hz streams** (VU today; a future spectrum analyzer
   or waveform stream belongs there too, not in `emit`).
4. **Backend owns cadence.** Anything the backend samples on its own timer
   (latency windows, Icecast stats) should emit on that timer — never make the
   frontend guess the producer's rhythm with a second timer.
