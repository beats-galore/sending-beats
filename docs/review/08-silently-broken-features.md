# Silently Broken Features & Correctness Bugs

**TL;DR — this vector wasn't in the original brief, but it's the most urgent
one.** Several user-facing features report success and do nothing, and a handful
of state-management bugs corrupt live behavior. These are the residue the
pipeline rewrites left behind: the new generation went in, the old feature's
wiring was stubbed "for now", and the stubs still return `Ok(())`.

## The headline list

| # | What the user sees | What actually happens |
|---|---|---|
| 1 | EQ / compressor / limiter knobs work | All 6 effects commands are `Ok(())` no-ops |
| 2 | "Go live" starts a broadcast | No TCP connection is ever opened; a 10 kHz task leaks |
| 3 | Un-soloing one of two soloed channels | Solo mode turns off globally |
| 4 | Removing an input device | Pipeline cleanup half-runs, always logs a failure |
| 5 | An MP3 recording finishes | Final frames + seek header never written |
| 6 | VU meters at 60 Hz | Meters update at ~10 Hz |
| 7 | Listener count | `get_listener_stats` reads a `StreamState` Tauri never manages — always errors |
| 8 | Editing recording metadata | Always `"Recording service not initialized"` — its init is never called |

## Details

### 1. The entire custom-effects surface is disconnected (three layers deep)

- **Command layer**: all six commands in `commands/audio_effects.rs` return
  `Ok(())` with their real bodies commented out beneath (`:13-53`, `:66-107`,
  `:117-151`, `:161-208`, `:217-256`, `:264-290`) — 249 of 291 lines are dead
  code referencing a `mixer.get_channel` API that no longer exists. All six are
  registered in `lib.rs:485-490`. `get_channel_effects` is declared
  `Result<(), String>` — a getter that returns nothing.
- **Coordinator layer**: `IsolatedAudioManager::handle_update_effects`
  (`isolated_audio_manager.rs:1327-1333`) is an empty stub returning `Ok(())`.
- **Worker layer**: `InputWorker.custom_effects` (`input_worker.rs:32`) is
  written by `update_custom_effects` but **never read by the processing
  closure** — post-processing applies only the default chain (gain/pan/mute/
  solo) and VU (`input_worker.rs:263-299`).

The DSP itself (`effects/`) is healthy and allocation-free. Only the wiring is
gone. **Decision needed:** reconnect (route commands → coordinator →
`update_custom_effects` → actually process `custom_effects` in the post-process
closure) or delete the surface honestly and unregister the commands.

### 2. Broadcasting cannot broadcast

The live path (`commands/icecast.rs:58` → `broadcasting/utils.rs:29` →
`manager.rs:239` → `icecast_source.rs:376`) ends in a task whose encode-and-send
is a `TODO` (`icecast_source.rs:407`). It sends `StreamControl::Start` into a
channel whose receiver only runs inside `IcecastStreamManager::run()` — **which
is never called anywhere**. After 16 on-air toggles the control channel is full
and `send().await` blocks forever. Additionally:

- `broadcasting/streaming.rs::AudioEncoder::encode_pcm_to_mp3` (`:326-344`)
  decodes PCM into a discarded `Vec<i16>` and **returns the raw PCM unchanged**
  while the stream is labeled `Content-Type: audio/mpeg`. A real LAME encoder
  exists in `recording/encoders.rs:315-380` — the job is duplicated and one copy
  is fake.
- `bridge.rs:294,300` would self-deadlock on a tokio Mutex if `run()` were ever
  wired up (doc 04 §13).

Three generations of streaming commands are all registered simultaneously
(`streaming.rs`, `icecast.rs`, `cast_configurations.rs::start_cast` — the last
one is what the UI actually calls). **Decision needed:** which generation
survives; the other two should be deleted, not fixed.

### 3. Solo state corruption

`InputWorker::update_solo` (`input_worker.rs:346-352`) writes the *last toggled
value* into the shared `any_channel_solo: AtomicBool`. Solo channels A and B,
then un-solo B → flag becomes `false` while A is still soloed → every muted
channel unmutes. The flag must be recomputed as an OR across all channels'
solo states (which requires one owner for channel state — see doc 01 §A8).

### 4. Input-device removal always errors (dead code sabotaging a live path)

`AudioPipeline::remove_input_device` (`pipeline_manager.rs:721-723`) calls
`queues.remove_input_device()` on the vestigial `PipelineQueues` — which nothing
ever populates (no caller of `queues.add_input_device` exists) — so it returns
`Err("not found")` and `?` aborts the function **after** the worker was stopped
but **before** `latency_probe.remove_device`, the device-count decrement, and
the mix-rate recalculation. The caller just warns
(`isolated_audio_manager.rs:1027-1029`). Every input removal takes this path.

Related in the same function family:

- **Removing the last device fails**: `calculate_target_mix_rate`
  (`pipeline_manager.rs:169-176`) errors when no devices remain, and both
  removal paths call it after removing (`:730`, `:792`) — state is mutated,
  then an error is returned.
- `get_sample_rate()` panics via `.expect()` (`pipeline_manager.rs:71-74`) and
  is reachable from `get_pipeline_stats` (`:911`) before any device is added.

### 5. MP3 recordings are truncated

`recording/encoders.rs:382-391`: `finalize()` never calls LAME's flush — the
final frames are lost and no Xing/LAME header is written, so players can't seek
the file and duration is wrong. (Contrast the WAV path, which finalizes
correctly via `finalize_patches` and even emits a valid empty file for
zero-sample recordings.)

### 6. VU meters run at one-sixth the configured rate

Both call sites configure 60 Hz; the emit loop sleeps a flat 100 ms when the
send interval hasn't elapsed (`vu_channel_service.rs:254-256`) → ~10 Hz actual.
One-line fix (`tokio::time::interval`).

## Second tier (real, lower urgency)

7. **Rubato's short-input guard is commented out** (`rubato.rs:174-183`): a
   caller that passes fewer frames than required gets silent zero-padding
   spliced into the stream instead of an error. Only accumulator discipline
   upstream prevents it today.
8. **First-run drain runs once per process, not per stream**
   (`coreaudio_stream.rs:820-827`): `FIRST_RUN` is a global static, so only the
   first-ever output stream gets the drain-to-latest behavior; later or
   restarted streams start with accumulated backlog. The companion guard at
   `:905` can never skip (the flag was already flipped) — dead logic.
9. **`stop()`'s sleeps guard nothing**: `is_running` is set false and slept on
   (`coreaudio_stream.rs:568-576`, `:1413-1421`), but neither callback ever
   reads `is_running`. 100 ms of pure blocking per stop, doubled by `Drop`
   re-calling `stop()`.
10. **Stereo hardcoded against its own channels field**: the render callback
    computes `target_samples = in_number_frames * 2` (`coreaudio_stream.rs:824`)
    while `samples_to_fill` correctly uses `context.channels` — non-stereo
    output devices mis-read the ring. Mono inputs also over-accumulate 2× in
    `process_with_pre_accumulation` (`audio_worker.rs:747-756`), adding latency.
11. **Stats theater** — every stats surface reports zeros or constants:
    `MixingLayerStats.mix_cycles/samples_mixed` (never updated;
    `mixing_layer.rs:138-139` vs. worker-local counters),
    `InputWorkerStats.samples_processed` (always 0) / `is_running` (hardcoded
    true, `input_worker.rs:127-135`), `OutputWorkerStats` likewise,
    `IsolatedAudioManager.metrics` (static zeros; `output_streams` is the length
    of a map nothing inserts into — `isolated_audio_manager.rs:161,1318`).
    Either wire them to the real counters (via atomics, like `latency_probe`)
    or delete them.
12. **Duplicate-start guards that never fire**: recording and Icecast
    duplicate checks consult the never-populated `output_rtrb_producers`
    (`isolated_audio_manager.rs:1354,1470`) — only the pipeline's own duplicate
    check downstream saves them.
13. **Bridge error-recovery eats control commands**
    (`broadcasting/bridge.rs:316-321`): two `try_recv` calls consume and discard
    up to two pending commands (a `Stop` vanishes); no reconnect is sent.
14. **`pick_shuffled` reseeds xorshift from `subsec_nanos()` per call**
    (`file_player/player.rs:713-734`) — correlated picks; hold a `SmallRng`.
15. **Listener stats structurally cannot work** —
    `commands/streaming.rs:7` defines its own `StreamState` while Tauri manages
    the different `lib.rs:58` type; every command in the file resolves to the
    unmanaged one. The frontend polls `get_listener_stats`
    (`use-listener-stats.ts:22`) and it can never succeed.
16. **Recording metadata updates always fail** — `RecordingService::initialize`
    is never called, so `update_recording_metadata` (frontend-invoked) errors
    unconditionally; the recording *command loop* and crash/temp-file recovery
    are dead with it (recording itself works via the coordinator's RTRB path).
17. **Five frontend `invoke()`s have no backend command** and throw at runtime:
    `add_mixer_channel`, `create_mixer_input_for_application`,
    `start_application_audio_capture`, `get_channel_levels`,
    `open_system_preferences_privacy` (backend fn exists, never wired).
18. **`.ogg` playback is likely broken** — `symphonia-codec-vorbis` is a direct
    dependency, but the `vorbis` feature isn't enabled on the `symphonia`
    facade, so the codec never registers with `default::get_codecs()`. Test
    before trusting the format list.
19. **Data-layer correctness** (details in doc 06): unscoped
    `delete_many` wipes a device from every saved configuration; channel-number
    assignment races; zero transactions in the command layer; a soft-delete
    removal migration resurrects deleted rows; the `audio_effects_custom` FK
    landmine that will break device removal the day the table gets its first row.
