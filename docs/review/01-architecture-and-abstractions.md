# Architecture & Abstractions

**TL;DR — the current pipeline generation is the right architecture; the problem
is that the previous generations were never buried, and a few abstractions are
shaped wrong for where the code is going.** The five highest-leverage moves:

1. **Delete the dead generations** (`PipelineQueues`, `VirtualMixer`, the tap
   module, 2 of 3 streaming stacks, the raw-sqlx db files) — they aren't just
   clutter, two of them corrupt live behavior (doc 08 §4, doc 06).
2. **Replace the `AudioWorker` trait with a struct + role config** — it's a
   20-method getter/setter trait with no dynamic dispatch, costing ~250 lines of
   delegation boilerplate.
3. **Fix `OutputWorker`'s swapped field semantics** — `device_sample_rate` holds
   the mix rate and `target_sample_rate` the hardware rate, with comments
   apologizing at every use site.
4. **Give channel state one owner** — gain/pan/mute/solo is smeared across the
   effects chain, worker fields, and a pipeline-level atomic; the solo bug
   (doc 08 §3) is a direct consequence.
5. **One registration ledger** — the coordinator and pipeline each track devices;
   one of the coordinator's two books is known-empty and still consulted.

## The layer map (as built)

```
Tauri commands (src/commands/*)            132 commands, Result<T, String>
        │ mpsc::Sender<AudioCommand>
        ▼
IsolatedAudioManager (coordinator)         owns AudioPipeline + StreamManager,
        │                                  serializes ALL audio commands
        ▼
AudioPipeline (pipeline_manager.rs)        registration, rates, lifecycle
  ├─ InputWorker  (per input)   ─┐         resample→effects→VU, own thread
  ├─ MixingLayer  (one thread)   ├─ rtrb   block accumulate → per-bus sum → dispatch
  ├─ OutputWorker (per output)  ─┘         resample→pace→hardware ring
  └─ LatencyProbe                          time-weighted occupancy gauges
StreamManager                              CoreAudio/SCK stream handles
        ▼
CoreAudio callbacks (coreaudio_stream.rs)  capture → rtrb; rtrb → render
```

This shape is sound: hardware callbacks touch only ring buffers; everything
between runs on deadline-scheduled threads; the mixer is paced by output drain.
The findings below are about what's *around* this core.

## A1. `AudioWorker` is a Java interface in a Rust codebase

`audio_worker.rs:156-229` — 20 trait methods of which ~17 are pure
getters/setters over `AudioWorkerState`, which both implementors already
contain. `InputWorker` and `OutputWorker` each carry ~90 lines of
`fn x(&self) { self.state.x() }` delegation (`input_worker.rs:150-238`,
`output_worker.rs:150-238`). There is no `dyn AudioWorker` anywhere — the trait
is never used polymorphically. The genuine variation is five items:
`work_period()`, `applies_backpressure()`, `inbound/outbound_channels()`,
`log_prefix()`, plus the post-process closure.

**Direction:** one `Worker` struct owning `AudioWorkerState` + a small
`WorkerRole { period, backpressure, channels_in/out, label }` value + the
existing `post_process_fn` hook. Deletes ~250 lines and removes a whole
category of "add a method in three places" maintenance.

## A2. `OutputWorker` swaps the meaning of its own fields

`output_worker.rs:65-77`: "OutputWorker receives samples at target_sample_rate
(mixing) and outputs at device_sample_rate (hardware). **So we swap the rates**"
— i.e. the field named `device_sample_rate` holds the mix rate. Every read site
then needs a comment (`:224-228`: "`target_sample_rate` is the hardware rate on
this side"). The drift-adjust call inherits the confusion
(`audio_worker.rs:771-777` parameter names vs. what's passed). Rename to
`source_rate`/`sink_rate` (direction-neutral) in `AudioWorkerState` and the
swap disappears as a concept.

## A3. Dead generations still wired into the live path

Full inventory in doc 02; the architectural point is that each rewrite left its
predecessor *installed*, and two of them still execute:

- **Generation 1 (tokio-mpsc pipeline)**: `queue_types.rs` — `PipelineQueues`
  is still a field of `AudioPipeline`, constructed at both `new()`s, and its
  `remove_input_device` is still *called* — which is why every input removal
  errors (doc 08 §4). `add_output_device` returns a receiver wired to nothing
  with a `TODO: We need a broadcast mechanism here`.
- **Generation ~1.5 (`VirtualMixer`)**: `virtual_mixer.rs:13-14` is
  `pub struct VirtualMixer {}` with an `async fn new()` returning the empty
  struct. The file header still claims "stream lifecycle management, device
  switching". Its real content is two static utilities that `bus_mixer` and
  `input_worker` reach across module boundaries to use, plus a dead auto-gain
  function. Move the utilities into `pipeline/`, delete the struct and its
  `audio/mod.rs` re-export.
- **Streaming ×3**: `commands/streaming.rs` (7 commands, own `StreamState`),
  `commands/icecast.rs` (9 commands), `cast_configurations::start_cast` (what
  the UI calls). All registered simultaneously in `lib.rs`. The middle one is
  also non-functional (doc 08 §2).
- **`tap/`**: honestly documents that process taps were abandoned for
  ScreenCaptureKit, yet ships 550+ lines of unreachable modules kept alive by
  one stray import (`system_audio_router.rs:2` imports `coreaudio_sys` symbols
  *via* `tap::core_audio_bindings` re-exports).

## A4. Constructor & parameter proliferation

- `AudioPipeline::new` / `new_with_hardware_updates` duplicate wholesale
  (`pipeline_manager.rs:76-129`).
- `OutputWorker` has two 10-11-arg constructors differing by one field
  (`output_worker.rs:35,89`); clippy flags four functions at 8-11 args.
- `add_input_device_with_consumer_and_producer` takes 10 params, four of them
  `Option`-typed initial channel state (`pipeline_manager.rs:251-263`) — that
  quadruple travels as a bare tuple through three more signatures
  (`channel_placement_for`, both add-stream handlers). Make it an
  `InitialChannelState` struct; make constructors take a config struct.
- Method-name-as-changelog: `add_output_device_with_rtrb_producer_and_tracker`,
  `new_with_rtrb_consumer_and_notifier` (there is no notifier anymore) — names
  describing plumbing history rather than role.

## A5. Duplication where a shared helper already exists

- The ~90-line channel-number + initial-effects DB lookup block appears
  verbatim in `handle_add_coreaudio_input_stream`
  (`isolated_audio_manager.rs:609-693`) and
  `handle_add_application_audio_input_stream` (`:789-865`) — while
  `channel_placement_for()` (`:971-1016`), which is exactly this logic, is
  called only by the file-player path. The refactor was done once and applied
  to one of three call sites.
- `handle_start_recording` ≈ `handle_start_icecast` (`:1340-1412`, `:1452-1517`)
  — same ring+tracker+add-output shape; parameterize as one "internal tap"
  registration.
- The "internal tap" concept itself is string-matching:
  `is_internal_output` checks `"recording_output"` and prefix
  `"icecast_output_"` (`:1088-1090`). Device identity deserves a type
  (`DeviceKind::{Hardware, AppCapture, FilePlayer, RecordingTap, CastTap}`)
  instead of naming conventions carried in `String`s across the whole pipeline.
- MP3 encoding ×2 (real in recording, fake in broadcasting — doc 08 §2);
  reconnect logic ×3, none with backoff (`manager.rs:590-636`,
  `bridge.rs:232-257`, `icecast_source.rs:494-500`); CFString property reads ×5
  with the retain-rule bug in 3 (doc 04 §5).

## A6. Two (three) sources of truth for device registration

The coordinator keeps `output_rtrb_producers` while the pipeline keeps
`output_workers`; the code itself documents the consequence: "Both books have
to be consulted" (`isolated_audio_manager.rs:1179-1185`). One book is
**known-empty** (nothing ever inserts — doc 08 §12) yet still drives duplicate
guards and metrics. `StreamManager` is the third ledger (hardware handles).
One registry, with the stream handle and worker handle as fields of one entry,
removes the whole class.

## A7. Channel state has no single owner

Gain/pan/mute/solo live in `DefaultAudioEffectsChain` (per worker), solo also
in a pipeline-level `Arc<AtomicBool>` written by whichever worker last toggled
(the doc 08 §3 bug), initial values travel as a 4-tuple of `Option`s from the
DB, and `custom_effects` sits unread next to them. A `ChannelStrip` owned by
the pipeline (workers hold handles) makes solo recomputation, persistence, and
the effects reconnection all one-place changes.

## A8. Bus registry forked at thread start

`MixingLayer` keeps a `BusMixer` whose registry is **cloned** into the mixing
thread at `start()` (`mixing_layer.rs:350`); after that, commands mutate both
copies but only the thread's copy matters. The layer-side copy exists "so the
layer keeps one it can still answer queries from" — but nothing queries it.
Combined with `mem::take` of the stream maps at start, a stop→start cycle
silently loses all connections (doc 04 §11). Make the thread the sole owner
post-start, or make restart re-register from the coordinator's state.

## A9. The dev loop hides all of this

`turbo rust:check` runs `RUSTFLAGS="-A warnings" cargo check`
(`package.json`/turbo task) — the standard loop suppresses **every** warning,
while `cargo clippy` (not part of `rust:check`) reports ~400, including the 14
`static mut` UB warnings and dozens of unused imports/variables that map
directly to the dead code in doc 02. The `unreachable_patterns` warning that
would have caught the double-registered recording commands (doc 06) was
suppressed the same way. **Recommendation:** drop `-A warnings` from
`rust:check`, or make `rust:clippy` part of the pre-commit gate; then burn the
warning list down with the dead-code deletions.

## A10. Smaller structural notes

- `audio/mod.rs:26-70` re-exports ~45 symbols in bulk, including entire dead
  families (`tap`, `StreamManager`(broadcast), `VirtualMixer`) — the re-export
  layer is what shields dead code from `dead_code` warnings. The project's own
  frontend rule ("never re-export") is the right instinct for the backend too.
- `lib.rs::run()` is 462 lines and spins up 3–5 tokio runtimes (`lib.rs:148-609`;
  DB init runtime, audio thread runtime, signal-handler runtime, Tauri's own,
  plus one in the panic hook). Extract `build_audio_state()` /
  `install_signal_handlers()`; prefer `tauri::async_runtime` over bespoke ones.
- Stale comments from the async generation survive in the thread generation
  (`audio_worker.rs:419-421` explains a lock scoped "across an await" in a
  non-async closure) — worth sweeping during the trait removal.
- Error vocabulary is three-way even inside one subsystem: `anyhow` (pipeline),
  `Result<_, String>` (rubato, all commands), `thiserror` (now_playing, BusError).
  Pick `thiserror` for domain modules + `anyhow` at composition points +
  a serializable `CommandError` at the Tauri boundary (doc 06).

## What the architecture gets right (worth protecting)

- The four-stage shape itself, and the decision that **callbacks only touch
  rings** — no locks, DB, or allocation dependencies from hardware threads
  (violations inside the callbacks are implementation, not architecture — doc 03).
- Pacing by downstream occupancy, not ring capacity (`audio_worker.rs:25-38`) —
  the key latency insight, documented where it lives.
- `bus_mixer` summing once per bus regardless of destination count, with real
  behavior-named tests.
- `latency_probe` as a stage-agnostic observability seam — the pattern to extend
  for all realtime metrics.
- The newer modules' comment discipline (pacing, block_accumulator, file_player,
  bus removal ordering) — comments explain *why*, and several encode invariants
  in tests. The delta between these and the older files is the clearest
  signal of which code is current.
