# Performance & Realtime Hygiene

**TL;DR — the architecture is right, the inner loops are not.** The pipeline's shape
(per-device worker threads with mach deadline scheduling, SPSC ring buffers, a
paced mixer) is genuinely good. What undermines it is what happens *inside* the
hot loops: heap allocations per cycle, per-sample atomic push/pop where chunked
slice access exists, `tracing` logging on realtime threads, and Mutex-wrapped
ring buffers whose lock failure paths drop audio. None of these need a redesign
— they are all local fixes to existing loops.

## Top 5 actions by impact

1. **Switch every rtrb access to chunked reads/writes** (`read_chunk`/`write_chunk`
   give you slices; current code pops/pushes one `f32` atomic op at a time).
2. **Remove allocations from the audio path** — pooled scratch buffers per stage;
   two "reusable buffer" optimizations currently end in `.clone()`, defeating
   themselves.
3. **Get `tracing` calls off realtime threads** — replace with atomic counters
   drained by a reporter task (the `latency_probe` already models this pattern
   correctly).
4. **Unwrap the `Arc<Mutex<…>>` around rtrb endpoints** — each end is single-owner
   by design; the mutexes exist only to satisfy ownership plumbing, and their
   `try_lock` failure paths silently drop audio or output silence.
5. **Fix the coordinator stalls** — ~250 ms of blocking `std::thread::sleep` per
   device removal runs on the async coordinator, freezing every queued audio
   command (gain changes, device switches) behind it.

---

## 1. Allocations on realtime threads

Every one of these runs per cycle, per block, or per hardware callback:

| Site | Allocation |
|---|---|
| `mixer/pipeline/mixing_layer.rs:561` | `Vec::with_capacity(available)` per device, per mix cycle |
| `mixer/pipeline/block_accumulator.rs:185,218` | `drain(..).collect()` into a fresh `Vec` + `device_id.clone()` (String) per device per block |
| `stream_management/virtual_mixer.rs:211` | thread-local reuse buffer… then `buffer.clone()` as the return value ("final allocation, but unavoidable for API" — it is avoidable: write into a caller buffer) |
| `mixer/resampling/rubato.rs:260` | `reusable_result_buffer.clone()` — the field is literally named for the allocation it was meant to eliminate |
| `mixer/pipeline/audio_worker.rs:761` | `drain(..n).collect()` per chunk; `device_id.clone()` per chunk at `:537` |
| `devices/coreaudio_stream.rs:806` | `input_samples: Vec` grown per render callback |
| `devices/coreaudio_stream.rs:1516` | `vec![0.0; total_samples]` per input callback (+ a second one on `AudioUnitRender` error) |
| `audio/vu_channel_service.rs:113-147` | `Arc::from(samples)` full-block copy per meter update, which `try_send` may then discard |

**Direction:** each stage owns pre-sized scratch buffers (the resampler already
pre-allocates its channel buffers — extend the pattern to its output). The
CoreAudio callback contexts should carry a reusable buffer in
`AudioCallbackContext` / `AudioInputCallbackContext`.

## 2. Per-sample atomic traffic on lock-free rings

`rtrb` supports chunk-based access returning contiguous slices. The codebase
pushes/pops **one sample at a time** — one atomic operation per `f32` — in:

- the mixing collection loop (`mixing_layer.rs:564-567`)
- the worker read loop (`audio_worker.rs:488-496`)
- `write_samples_to_rtrb_sync` (`audio_worker.rs:309-356`)
- `bus_mixer.rs::write_block` (`:197-222`)
- both CoreAudio callbacks (`coreaudio_stream.rs:831-899`, `:1574-1586`)

At 48 kHz stereo that is ~96,000 atomic RMWs per second per queue that could be
a handful of `memcpy`s. This is the single cheapest large win in the codebase.

## 3. Logging on realtime threads

`info!`/`warn!` (with `colored` formatting) execute inside:

- the mix loop (`mixing_layer.rs:576,709,723,741`)
- worker processing loops (`audio_worker.rs:583,636,650`)
- **both CoreAudio callbacks** (`coreaudio_stream.rs:728,776,785,847,931,965,1559-1597`)
- the ScreenCaptureKit delivery callback (`screencapture/stream.rs:150-166,179-204`)

`tracing` can take a subscriber lock and do I/O; on a CoreAudio realtime thread
that is a priority-inversion risk. The irony is explicit: the render callback
logs `COREAUDIO_CALLBACK_DELAY` warnings measuring delays its own logging
contributes to. Rate-limiting (`% 1000`) reduces frequency but not the worst
case, and the rate-limit counters themselves are `static mut` (see doc 04).

**Direction:** counters + gauges drained by a reporter thread. `latency_probe.rs`
already implements exactly this pattern for occupancy — extend it to event
counters (drops, underruns, contention).

## 4. Mutexes around SPSC endpoints

Every rtrb `Producer`/`Consumer` is wrapped in `Arc<Mutex<…>>`
(`audio_worker.rs:48-49`, `mixing_layer.rs:104-110`, `coreaudio_stream.rs:301`).
rtrb endpoints are single-owner by design — the type system already guarantees
exclusive access. The mutexes exist only because ownership is threaded through
command channels, and they cost:

- an atomic RMW per lock per access on the hot path (uncontended, but nonzero);
- **failure paths that lose audio**: `write_samples_to_rtrb_sync` drops the whole
  batch on `try_lock` failure (`audio_worker.rs:348-355`); the render callback
  outputs silence (`coreaudio_stream.rs:762,977-979`).

**Direction:** move endpoint ownership into the thread that uses it (the command
channel can carry the endpoint itself, as `MixingLayerCommand::AddInputStream`
already does) and drop the mutexes.

## 5. Coordinator stalls (cascading delays)

The `IsolatedAudioManager` loop serializes all audio commands. Things that block it:

- `CoreAudioOutputStream::stop()` / input `stop()`: **two unconditional 50 ms
  `std::thread::sleep`s each** (`coreaudio_stream.rs:576,611,1421,1455`) — and
  `Drop` calls `stop()` again, sleeping twice more. ~200 ms per device removal.
  The sleeps claim to let callbacks observe `is_running` — **no callback ever
  reads `is_running`** (see doc 08 §9).
- `stream_manager.rs:76`: 150 ms blocking sleep replacing an existing input stream.
- DB lookups (`sea_orm`) awaited inline during device attach
  (`isolated_audio_manager.rs:609-693`) — reasonable, but they queue behind and
  in front of hardware-buffer-resize commands on the same channel.
- `system_audio_router.rs:120-132`: up to 2 s of `std::thread::sleep` on the
  async runtime (should be `tokio::time::sleep`).
- `screencapture::get_available_applications()`: synchronous FFI documented to
  take up to 10 s, called from `async fn`s without `spawn_blocking`
  (`commands/application_audio.rs:33,256`, `commands/device_attachment.rs:182`).

While the coordinator is blocked, a user's gain change sits in the queue: the
knob feels laggy exactly when the app is doing device work.

## 6. Head-of-line blocking in the mixer (by design — needs a watchdog)

The mixer produces only when **every** registered output can take a full block
(`mixing_layer.rs:648-670`). The comment explains why (overproduction is heard as
crunch), and the removal path correctly detaches producers before stopping
workers. But there is no defense against a *wedged* output: a worker that is
alive while its device stalls holds the entire mix — all outputs — at a
standstill. Recommend per-output staleness detection (no drain in N periods →
evict producer, log, keep mixing).

## 7. Drift controller (PI) — windup and an unverified sign

`queue_manager.rs::adjust_ratio`:

- **No anti-windup**: `integral_error` accumulates unbounded. A stall or device
  sleep accumulates a huge integral that pins the correction at the ±1% clamp
  long after recovery, mis-resampling until it unwinds.
- **Direction of correction deserves a test**: occupancy above target raises the
  output/input ratio. With `SincFixedOut` (fixed output per chunk), production
  rate ≈ input arrival rate × ratio — raising the ratio when the downstream
  queue is over target *increases* production. Stability currently appears to
  rest on the sampling instant (called right after a chunk write) and on the
  clamp + the block accumulator's floor-shedding masking any runaway. A
  property test simulating a fast/slow consumer would settle it.
- `adjust_dynamic_sample_rate` calls `resampler.set_sample_rates` **every chunk**,
  allocating a `String` device id each call (`audio_worker.rs:569-575,780-789`).

## 8. Resampler cost vs. latency budget

`rubato.rs:88-94`: `sinc_len: 256`, `oversampling_factor: 160`,
`BlackmanHarris2` — broadcast-quality settings. Group delay ≈ 128 frames
(~2.7 ms @ 48 kHz) **per pass**, and the common path resamples twice
(input→mix, mix→output): ~5 ms+ of standing latency plus significant CPU per
device. For the live monitoring path, a lighter profile (shorter sinc or
`FastFixedOut`) would meaningfully cut latency; keep the HQ profile for the
recording/broadcast taps where latency is irrelevant.

## 9. Smaller items

- **Double-buffering in workers**: samples are popped into `samples_buffer`, then
  copied into `input_accumulator` (`audio_worker.rs:467-525`) — pop directly
  into the accumulator.
- **String-keyed hot maps**: `HashMap<String, …>` lookups and `String` clones per
  block throughout the mixer/accumulator/bus path. Intern device ids to small
  integers (or `Arc<str>`) at registration; keep `String` at the edges.
- **Timing instrumentation always-on**: 4–6 `Instant::now()` per mix cycle and
  per worker chunk even when nothing logs (`mixing_layer.rs`, `audio_worker.rs`,
  `virtual_mixer.rs`). Cheap individually; gate behind a debug flag.
- **VU meter cadence bug**: both call sites configure 60 Hz, but the service
  sleeps a flat 100 ms when the interval hasn't elapsed
  (`vu_channel_service.rs:254-256`) → **actual meter rate ~10 Hz**. Use
  `tokio::time::interval`.
- **`stereo_levels` allocates two `Vec`s per block** to deinterleave before
  peak/RMS (`vu_channel_service.rs:35-45`) — computable in one pass, zero alloc.
- **Regexes recompiled per call** in `recording/filename_generation.rs:127,391`
  — `LazyLock` them.
- **Redundant bounds checks** in the mix inner loop
  (`virtual_mixer.rs:148-153`): index guards inside a loop already bounded by
  `min()`; iterator zips would also vectorize better.
- **Wrong constant in diagnostics**: callback-delay math hardcodes 44100 Hz
  (`coreaudio_stream.rs:719`) — expected-interval numbers are wrong for 48 kHz
  devices.

## What is already right (keep it)

- `realtime_thread.rs` — mach `thread_time_constraint_policy` with the timebase
  conversion done correctly, advisory-refusal handled, tested.
- Pacing-by-occupancy (produce when the consumer is *short*, not when the ring
  has room) — the single most important latency decision in the pipeline, made
  correctly and documented in `audio_worker.rs:415-446` and `mixing_layer.rs:627-651`.
- `block_accumulator.rs` floor-shedding — distinguishes bursts from standing
  backlog and sheds only the latter; this is what keeps long-session latency flat.
- `pacing.rs` expressing cushions as durations, not buffer multiples.
- Backpressure asymmetry — outputs hold back, capture sources never do
  (`audio_worker.rs:221-229`); exactly right for callback-fed sources.
- `effects/` DSP is allocation-free in the process path, with denormal/non-finite
  flushing (`effects/mod.rs:24-38`).
