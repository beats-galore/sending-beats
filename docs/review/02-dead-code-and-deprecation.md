# Dead Code & Deprecation Inventory

**TL;DR — the suspicion is correct, and it's worse than clutter: two dead
structures are still *called* from live paths (one breaks input-device removal,
one neuters duplicate-start guards), the re-export layer in `audio/mod.rs` and
`db/mod.rs` is what shields most of the graveyard from `dead_code` warnings,
and `-A warnings` in the standard check hides the rest.** Deleting the items
below removes roughly 4,500+ lines, most of the ~400 clippy warnings, and two
live bugs.

## Generation map (how the strata line up)

| Generation | What it was | What remains | Status |
|---|---|---|---|
| Gen 1: tokio-mpsc pipeline | `PipelineQueues` + `Raw/Processed/MixedAudioSamples`, per-layer unbounded channels | `queue_types.rs` (170 lines), `queues` field in `AudioPipeline`, `send_input_audio` | **Dead but still called** — removal bug (doc 08 §4) |
| Gen ~1.5: `VirtualMixer` | The mixer object itself | Empty struct + 2 static utilities + dead auto-gain (`virtual_mixer.rs`, 227 lines) | Hollow fossil, re-exported as a headline type |
| CoreAudio process taps | `tap/` module family | `virtual_stream.rs` (270), `core_audio_bindings.rs` (282), `types.rs` error enum, registry static | Abandoned for ScreenCaptureKit; kept alive by one stray import |
| Streaming gen 1 | `commands/streaming.rs` + `StreamState` | 7 registered commands; only `get_listener_stats` called by UI | Mostly dead; carries 9 production `unwrap`s |
| Streaming gen 2 | `commands/icecast.rs` + `IcecastStreamManager` | 9 registered commands; `run()` never called; encoder is a stub | **Non-functional** (doc 08 §2) |
| Streaming gen 3 | `cast_configurations::start_cast` | The live path | Current |
| Raw-sqlx db layer | `db/recordings.rs` (343), `db/broadcasts.rs` (323) | Zero callers; queries broken at runtime; plaintext password column | Dead **and** the convention it encodes is wrong (doc 06 H1) |
| Old effects command layer | `commands/audio_effects.rs` | 6 registered no-op commands, 249/291 lines commented out | Facade over a removed API (doc 08 §1) |

## Dead but still executing (delete first — these have blast radius)

1. **`PipelineQueues`** — `pipeline/queue_types.rs`. Nothing calls
   `add_input_device`/`add_output_device` (the latter returns a receiver wired
   to nothing, with `TODO: We need a broadcast mechanism here`), yet
   `AudioPipeline::remove_input_device` still calls
   `queues.remove_input_device()` (`pipeline_manager.rs:721-723`) → always
   `Err` → aborts cleanup mid-way, every time. Also drags `send_input_audio`
   (`:642-667`) and the `RawAudioSamples` import chain with it.
2. **`IsolatedAudioManager.output_rtrb_producers`** — never inserted into
   (producers are handed raw to `OutputWorker`, per the comment at
   `isolated_audio_manager.rs:1243`), but consulted by duplicate-start guards
   (`:1183,1354,1470`), removed-from in five places, and read for
   `metrics.output_streams` (always 0).
3. **`InputWorker.custom_effects`** + `handle_update_effects` +
   `commands/audio_effects.rs` — the three-layer no-op facade (doc 08 §1).
   Decision needed before deleting: is the feature coming back?

## Dead modules (unreachable or unconstructed)

- **`tap/`**: `VirtualAudioInputStream`, `ApplicationAudioInputBridge`,
  `CoreAudioTapCallbackContext`, `get_virtual_input_registry` (a static
  registry nothing inserts into), `ApplicationAudioError` + its `Result` alias
  — re-exported from `audio/mod.rs:59-60`, never constructed anywhere.
  `tap/manager.rs` honestly returns empty state ("abandoned for
  ScreenCaptureKit"). The only live edge is `system_audio_router.rs:2`
  importing `coreaudio_sys` symbols *via* `tap::core_audio_bindings`
  re-exports — repoint that import and the module family deletes cleanly.
  `process_discovery.rs` stubs: `get_cached_audio_applications` (returns
  empty + TODO), `is_app_playing_audio` (always false), `get_app_icon_path`
  (always None).
- **`db/recordings.rs` + `db/broadcasts.rs`** (666 lines): zero external
  references; all six runtime `query_as` calls would fail with
  `ColumnNotFound` on first use; glob re-exports at `db/mod.rs:22,31` mute the
  lint. Their six backing tables are orphaned too.
- **`broadcasting/streaming.rs`** (`StreamManager`/`AudioEncoder`): encoder
  returns PCM as "MP3", control channel stillborn, threads detached — its only
  caller is the also-unwired `bridge.rs::run()`. `broadcasting/service.rs` is
  a 15-line pure re-export file.
- **`mixer/stream_management/virtual_mixer.rs`**: keep
  `convert_mono_to_stereo` + `mix_input_samples_ref` (move into `pipeline/`),
  delete the struct, `async fn new()`, `apply_auto_gain_reduction`, and the
  `audio/mod.rs` + `stream_management/mod.rs` re-exports.

## Dead functions / fields / params (in otherwise-live files)

- `pipeline_manager.rs`: `send_input_audio`; `new()` vs
  `new_with_hardware_updates` (only the latter is used on macOS);
  `initialize_sample_rate` returns a `Result` that can never be `Err`.
- `queue_manager.rs`: `QueueInfo::new` / `on_samples_written` /
  `on_samples_read` / `update_derived_fields` — private and uncalled
  (`get_queue_info` constructs directly).
- `audio_worker.rs`: `_input_samples` param of `process_with_pre_accumulation`;
  duplicate `get_queue_tracker_for_consumer` (trait + inherent);
  stale async-era comment at `:419-421`.
- `coreaudio_stream.rs`: `send_audio()` + `input_buffer` field +
  `AudioCallbackContext.buffer` (the pre-rtrb path; zero callers);
  the dead `FIRST_RUN` skip-guard at `:905`; non-macOS placeholder impl.
- `input_worker.rs`/`output_worker.rs`: stats fields never incremented
  (`samples_processed`, `chunks_processed`, `samples_output`),
  `processing_time_total`; `QueueInfo` struct in `output_worker.rs:260-265`
  (shadows the queue_manager one, also unused).
- `mixing_layer.rs`: `mix_cycles`/`samples_mixed` fields (worker keeps locals).
- `isolated_audio_manager.rs`: `_producer` param of
  `handle_add_application_audio_input_stream` (callers allocate a ring pair
  whose producer is discarded); dead `buffer_capacity` computation at
  `:783-784`; doubled `#[cfg(target_os = "macos")]` at `:1150-1152`.
- `recording/encoders.rs`: `separate_channels` (`:274-288`) — `encode()`
  inlines its own copy.
- `broadcasting/manager.rs`: `connect_mixer` / `connect_mixer_ref` (bodies
  100% commented out), `run_audio_encoder` (never called, builds and discards
  an encoder); `bridge.rs:37` `conversion_buffer` (allocated 96 KB, never
  touched).
- `devices/monitor.rs:370-445`: `attempt_device_recovery` /
  `recover_device_stream` fully commented out — the recovery half of the
  monitor is a no-op that still ticks its interval.
- Commands: `check_screen_recording_permission` (`#[tauri::command]` never
  registered), `browse_audio_files` + `start_device_monitoring` placeholders,
  `get_channel_effects` returning `()`; dead `configuration_id` params in 3 of
  4 effects-default commands; `config_uuid` parsed and dropped
  (`configurations.rs:560`); `AudioDatabase::pool()`;
  `permissions`: `check_via_tccutil` (always `Err`), `request_permissions`
  (always `Ok(true)`), the `static mut` singleton itself.
- `lib.rs`: 8 recording commands registered twice (`:535-542` ≡ `:577-584`);
  `commands/mod.rs:23-40` re-export block (nothing imports through it).

## Duplicate implementations (consolidation candidates)

| What | Copies | Keep |
|---|---|---|
| MP3 encoding | `recording/encoders.rs` (real LAME) vs `broadcasting/streaming.rs` (fake) | recording's; share it |
| Reconnect loops | `manager.rs:590`, `bridge.rs:232`, `icecast_source.rs:494` — all different, none with backoff | one, with backoff |
| CFString property read | ×5 across `coreaudio_integration.rs` / `system_audio_router.rs` (3 leak) | one helper |
| Channel+effects DB lookup | ×3 in `isolated_audio_manager.rs` (helper exists, used once) | `channel_placement_for` |
| Configuration child-row cloning | ×9 blocks in `seaorm_services.rs` | one `clone_children` |
| Device-by-identifier lookup | ×4 in commands, different filters (doc 06 H2) | one scoped service fn |
| `QueueInfo` struct | `queue_manager.rs` + `output_worker.rs` | queue_manager's (or neither) |
| Channel deinterleave | `encoders.rs::separate_channels` + inline copy | inline, delete fn |

## Why the compiler didn't catch this

Three mufflers, all self-inflicted:

1. `RUSTFLAGS="-A warnings"` on `turbo rust:check` — the loop everyone runs.
2. **Glob re-exports**: `pub use` in `audio/mod.rs` (~45 symbols),
   `db/mod.rs:22,31`, `commands/mod.rs`, `broadcasting/mod.rs`,
   `recording/mod.rs`, `tap/mod.rs` — a re-exported symbol counts as "used".
3. `#[tauri::command]` + `generate_handler!` registration counts commands as
   used even when no frontend code ever invokes them.

**Recommendation:** delete top-down (module → re-export → symbols), re-enable
warnings, then run `cargo +nightly udeps` (or `cargo machete`) for dependency
pruning, and diff the `generate_handler!` list against actual frontend
`invoke(...)` strings as a CI check.

## Whole files confirmed dead (cross-reference pass)

- `db/broadcasts.rs` (323) and `db/recordings.rs` (343) — only references are their
  own definitions; shielded by `pub use *` globs at `db/mod.rs:22,31`.
- `audio/effects/analyzer.rs` (210) — `AudioAnalyzer` has zero callers;
  `PeakDetector`/`RmsDetector`/`SpectrumAnalyzer` live only as its fields.
  **Sole user of the `rustfft` dependency.** The live metering path is
  `vu_channel_service.rs` — the analyzer is the leftover.
- `audio/tap/virtual_stream.rs` (270) — all three types + the registry appear
  only at definitions and re-exports.
- `audio/devices/monitor.rs` (469) — **never runs**: the `DEVICE_MONITOR`
  OnceCell is never `set()`, `DeviceMonitor::new` never called. Its job
  (hotplug recovery) was superseded by `device_watcher.rs`, which *is* wired
  in. Consequence: `get_device_monitoring_stats` always returns `None`.
- `commands/audio_effects.rs` (291) — six `Ok(())` no-ops; **three are still
  invoked by the frontend** and silently succeed.

## Dead command/data chains (transitively dead end to end)

1. **Gen-1 queue pipeline** — `queue_types.rs` (169): `add_input_device` never
   called → every getter dead → and the `remove_input_device` call at
   `pipeline_manager.rs:721-723` always errors (the doc-08 §4 bug).
2. **Hardware-buffer-resize chain** — `OutputWorker.hardware_update_tx` is
   stored (`output_worker.rs:28,132`) but **never sent on** → 
   `UpdateOutputHardwareBufferSize` never constructed → the coordinator's
   `hardware_future` select arm never fires → 
   `StreamManager::update_coreaudio_output_buffer_size` → 
   `set_dynamic_buffer_size` all dead. The "dynamic hardware sync" feature is
   plumbing with no water in it.
3. **`UpdateEffects` / `GetAudioMetrics` commands** — the enum variants are
   never constructed anywhere; their handlers (one an empty stub, one reading
   write-only metrics) are unreachable. `stream_management::AudioMetrics` is
   transitively dead with them.
4. **Old streaming bridge** — `create_streaming_bridge`'s only live-looking
   references are commented out in `manager.rs`; `StreamingService.streaming_bridge`
   is set `None` and never touched; `streaming_stats` is written only in
   commented code, so the status's `audio_stats` is permanently `None`.
   `StreamingService.mixer` field: only reference is commented out.
5. **`VirtualMixer::new()` never called** — `AudioState.mixer` (`lib.rs:61`) is
   initialized `None` and **never assigned** — the struct survives only as a
   namespace for two static helpers. Everything gated on
   `audio_state.mixer.is_some()` is unconditionally dead (see zombies).
6. **Recording command loop** — `RecordingService::initialize` is never called
   → `command_sender` always `None` → `RecordingCommand` never constructed →
   the command loop and crash/temp-file recovery (`RecordingWriterManager::initialize`)
   never run. Recording works only through the coordinator's RTRB path.
7. **Device-health chain** — `initialize_device_health`, `check_device_health`,
   `should_avoid_device`, `get_health_statistics` reachable only from the dead
   `monitor.rs` (or tests).
8. **Non-macOS stubs inside `#[cfg(macos)]` modules** — 
   `coreaudio_stream.rs:1617-1646` and `core_audio_bindings.rs:257-260` can
   never compile in any configuration. `AudioPipeline::new()` is reachable only
   in non-macOS builds.

## Zombie commands (registered, some invoked, structurally cannot work)

| Command | Why it can't work | Frontend calls it? |
|---|---|---|
| `get_listener_stats` | `commands/streaming.rs:7` defines its **own** `StreamState`; the one Tauri manages is `lib.rs:58`'s — the command's state is never managed, so it always errors | **Yes** (`use-listener-stats.ts:22`) |
| `update_recording_metadata` | always `"Recording service not initialized"` (chain 6) | **Yes** |
| 6 effects commands | `Ok(())` no-ops | **Yes** (3+) |
| `start_device_monitoring` | gated on always-`None` `AudioState.mixer`; self-described placeholder | — |
| `get_device_monitoring_stats` | `DEVICE_MONITOR` never set → always `Ok(None)` | — |
| tap-family commands (`get_tap_statistics`, `get_active_audio_captures`, `stop_all_audio_captures`, `stop_application_audio_capture`, `cleanup_stale_taps`) | back the abandoned tap manager stub — return empty/`Ok(())` | 3 of 5 |

## Frontend `invoke()`s with **no backend command** (throw at runtime)

- `add_mixer_channel` — `services/mixer-service.ts:23` (a comment already knows)
- `create_mixer_input_for_application` — `stores/application-audio-store.ts:228`
- `start_application_audio_capture` — `stores/application-audio-store.ts:180`
- `get_channel_levels` — `services/audio-service.ts:20`
- `open_system_preferences_privacy` — `use-startup-permission-check.ts:29`
  (backend has `open_privacy_settings`, never wired to a command)

## Frontend-orphaned commands (registered, never invoked): 35

`browse_audio_files`, `cleanup_stale_taps`, `connect_to_stream`,
`create_audio_bus`, `create_reusable_configuration`,
`disable_system_audio_capture`, `disconnect_from_stream`,
`enable_system_audio_capture`, `get_application_info`, `get_audio_device`,
`get_configuration_by_id`, `get_configured_audio_devices_by_config`,
`get_debug_log_config`, `get_device_monitoring_stats`,
`get_file_player_devices`, `get_now_playing`, `get_stream_status`,
`get_supported_audio_formats`, `get_tap_statistics`,
`is_now_playing_watch_running`, `list_audio_buses`,
`refresh_audio_applications`, `remove_audio_bus`, `set_debug_log_config`,
`set_input_bus_sends`, `set_output_audio_bus`, `set_output_stream`,
`shutdown_application_audio_manager`, `start_device_monitoring`,
`start_icecast_streaming`, `start_streaming`, `stop_now_playing_watch`,
`stop_streaming`, `update_metadata`, `validate_audio_file` — plus
`check_screen_recording_permission`, which is written but never registered.

*(Some of these are bus commands the UI reaches via other wrappers — prune
against intent, not mechanically.)*

## Unused Cargo dependencies

Zero references anywhere (verified against every `.rs` incl. `build.rs`):
**`spmcq`**, **`crossbeam`** (crossbeam-channel *is* used), **`url`**,
**`futures`**, **`async-trait`**, **`flac-bound`** (the FLAC encoder is a stub
returning "not yet implemented"), **`core-foundation-sys`**, **`coreaudio-rs`**
(the used crate is `coreaudio-sys`), **`symphonia-format-ogg`** (already in
symphonia's features), **`windows`**, **`alsa`**, **`pulse`**; dev-deps
**`tokio-test`**, **`mockall`**, **`proptest`**, **`serial_test`**
(`tempfile` is used).

Becomes unused after the deletions above: **`rustfft`** (analyzer only),
**`objc2` / `objc2-core-audio` / `objc2-foundation`** (dead half of
`core_audio_bindings.rs` only).

⚠️ **`symphonia-codec-vorbis`**: depending on the codec crate directly does
*not* register it with `symphonia::default::get_codecs()` — the `vorbis`
feature isn't on the `symphonia` facade, so **`.ogg` playback likely doesn't
work**. Test before deleting; this may be a bug, not just an unused dep.

## Notable dead symbols in live files (selection)

`AudioDatabase::pool()` · `audio/types.rs` `AudioMetrics` +
`create_streaming_config` · all 15 `entities/mod.rs` re-exports ·
`pipeline_manager` `set_hardware_update_channel`/`send_input_audio`/
`get_pipeline_stats`/`PipelineStats` · both `QueueInfo`s +
`get_queue_info` · rubato's `conversion_needed`/`get_input_frames`/
`get_output_frames`/`get_current_ratio` · `get_queue_tracker_for_consumer`
(×3) · `get_default_effects`/`get_custom_effects_mut` ·
`default_effects_chain::{is_solo, process_mono}` · `VULevelEvent::new_mono` ·
broadcasting `normalize_audio`/`finalize_mp3`/`set_metadata`/`is_streaming`/
`start_streaming`(util)/`create_stream_bitrate_preset` ·
`device_manager::refresh_devices` · `VirtualDriverManager::uninstall` ·
`system_audio_router::get_current_default_output_uid` ·
`permissions::{open_privacy_settings, reset_permissions}` · `log.rs`
`audio_debug!` family · 13 uncalled methods on `RecordingService` + 7 on
recording `types.rs` + 4 on `silence_detection` · ~35 of `audio/mod.rs`'s
re-exported names never reached through the facade (call sites use canonical
paths) · `#![allow(dead_code)]` at `core_audio_bindings.rs:7` — the only
allow in the backend, hiding the tap FFI graveyard.

## Needs an owner call before deleting

- `AudioState.mixer` — deleting commits to "no future mixer object in Tauri
  state"; it has been `None` forever.
- `monitor.rs` — 469 lines of never-run health/recovery; `device_watcher.rs`
  superseded it. Reviewer reads it as a dead generation.
- The six CoreAudio FFI symbols `system_audio_router` borrows from
  `tap/core_audio_bindings.rs` — relocate to `audio/devices/` so the rest of
  `tap/` deletes cleanly.
- `.ogg`/vorbis support (above) — test, then either fix the feature flag or
  drop the codec dep.
