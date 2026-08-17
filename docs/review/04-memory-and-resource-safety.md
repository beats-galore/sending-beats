# Memory, Threads & Resource Safety

**TL;DR — three real leaks, two spinning tasks that never die, one class of
undefined behavior, and several threads with no owner.** The most dangerous
single item is the `static mut` diagnostic state inside the CoreAudio callbacks:
with two devices it is a data race (UB), not a style issue. The most
consequential leak is ScreenCaptureKit's callback context (one ring buffer per
start/stop cycle). The broadcast path leaks an unkillable 10 kHz task per
"go live" — and it can't be fixed in isolation because that whole path is
non-functional (see doc 08).

## Severity-ordered findings

### UB-class

1. **`static mut` mutated from realtime callbacks** —
   `devices/coreaudio_stream.rs`. Render callback: `LAST_CALLBACK_TIME`,
   `CALLBACK_COUNT`, `TOTAL_DELAY_ACCUMULATION` (`:705-707`),
   `LOCK_CONTENTION_COUNT`, `TOTAL_LOCK_TIME` (`:766-767`),
   `QUEUE_READ_START_TIME`, `TOTAL_QUEUE_UNDERRUNS`, `LAST_QUEUE_SIZE`
   (`:809-811`), `RTRB_PLAYBACK_COUNT` (`:958`). Input callback: `ERROR_COUNT`,
   `LAST_ERROR_TIME` (`:1544-1545`), `INPUT_CAPTURE_COUNT` (`:1588`).
   Each stream's callback runs on its own CoreAudio realtime thread; **two
   output devices = concurrent unsynchronized writes to the same statics =
   data race**. `Option<Instant>` is a 16-byte non-atomic write — torn reads
   are real. These also cross-contaminate diagnostics between devices. Clippy
   flags all 14. Fix: per-stream counters living in the callback context
   (`AudioCallbackContext`), plain `AtomicU64` for anything genuinely global.
   Same pattern at `permissions/mod.rs:206-217` (`static mut` singleton over a
   zero-sized struct — delete it or `OnceLock`; `static_mut_refs` is a hard
   error in Rust 2024).

2. **Unchecked negative length at an FFI boundary** —
   `screencapture/stream.rs:171`: `from_raw_parts(samples, sample_count as usize)`
   with a null check on the pointer but none on the `i32` count. A negative
   count from the Swift side becomes a ~2^64-length slice. One
   `if sample_count <= 0 { return; }` closes it.

3. **CoreAudio qualifier passed as a Rust wrapper struct** —
   `system_audio_router.rs:253-254`, `coreaudio_integration.rs:535`:
   `&cf_uid as *const _ as *const c_void` where `cf_uid: CFString` relies on an
   undocumented single-field layout. Use `as_concrete_TypeRef()` (the unused
   `TCFType` import at `system_audio_router.rs:237` suggests it used to).

### Leaks

4. **ScreenCaptureKit `StreamContext` leaked per start** —
   `screencapture/stream.rs:48-52`: `Box::into_raw` on start; the only
   `Box::from_raw` is the failure path. `stop()` and `Drop` never reclaim it —
   each start/stop cycle permanently leaks the context **and the
   `Arc<Mutex<rtrb::Producer<f32>>>` ring buffer inside it**. Caution: the leak
   is currently what prevents a use-after-free, because `sc_audio_stream_destroy`
   doesn't guarantee callback quiescence. The fix needs a Swift-side completion
   (or an Arc whose last ref drops after quiescence), not a naive `from_raw`
   in `stop()`.

5. **CFString leaked on every device enumeration** —
   `coreaudio_integration.rs:178,437,518`: `wrap_under_get_rule` on properties
   returned under the **create rule** (caller owns +1) → net +1 forever, per
   device name/UID, per enumeration, and enumeration runs on every hotplug
   burst. The correct form exists 200 lines away
   (`system_audio_router.rs:165`, `wrap_under_create_rule`). A single
   `get_cfstring_property()` helper would make the inconsistency impossible —
   the same boilerplate is copy-pasted five times.

6. **`sc_audio_free_applications` skipped for empty non-null arrays** —
   `screencapture/discovery.rs:35-38`. Small, bounded.

### Tasks & threads with no owner

7. **Icecast source task: unkillable 10 kHz spinner** —
   `broadcasting/icecast_source.rs:376-423`: `loop {}` with no shutdown signal,
   no stored `JoinHandle`, `sleep(100µs)` when idle, and a `TODO` where
   encode-and-send should be. `stop_streaming()` aborts only the monitor task
   (`manager.rs:215-217`). Every "go live" leaks one of these for the process
   lifetime. (The path is also non-functional — doc 08 §2.)

8. **Recording consumer task never exits after stop** —
   `recording/recording_service.rs:159-240`: the loop only learns the session
   ended if samples keep arriving; after stop the ring goes silent, so it takes
   the idle branch — `sleep(100µs)` — **spinning at 10 kHz forever**. One leaked
   wake-loop accumulates per stopped recording.

9. **`streaming.rs` spawns two detached `std::thread`s per `start_stream` call**
   (`broadcasting/streaming.rs:129,144`) with no handles, exit condition an
   empty-`Vec` sentinel; its control channel is stillborn (`_control_tx` dropped
   at creation, `:107` — the receive arm fires `None` immediately). The caller
   (`bridge.rs:294`) would invoke this per 100 ms chunk if the bridge ran.

10. **Device monitor: no handle + restart race** — `devices/monitor.rs:180,189`:
    stop only stores `false`; a stop→start before the loop notices leaves two
    monitor loops sharing one stats mutex. *(Mitigating context from the
    cross-reference pass: the monitor never actually starts — `DEVICE_MONITOR`
    is never set — so this is a latent defect in a dead module; the deletion in
    doc 02 resolves it.)*

11. **MixingLayer restart loses every stream** — `mixing_layer.rs:346-350`:
    `start()` `mem::take`s consumers/producers into the thread closure; after
    `stop()` joins, they are dropped. A stop→start cycle silently runs with zero
    streams. Currently masked because stop only happens at shutdown.

12. **`VUChannelService::Drop` detaches instead of aborting**
    (`vu_channel_service.rs:281-285`) — benign 100 ms zombie, but `shutdown()`
    and `Drop` should do the same thing.

### Deadlock / poisoning

13. **Self-deadlock landmine in the bridge** — `broadcasting/bridge.rs:294,300`:
    a `tokio::Mutex` guard held by the `match` scrutinee is still alive when the
    arm re-locks the same mutex. Not reentrant → permanent hang. Currently
    unreachable (`run()` has no callers) — which is exactly why it will bite
    whoever wires the bridge up.

14. **Poisoning as a systemic risk**: `broadcasting/streaming.rs` has 10
    `status.lock().unwrap()` sites; `file_player/player.rs` ~60. One panic while
    a guard is held (e.g. `resampled[0]` on an empty rubato result,
    `player.rs:597`) poisons the mutex, and every later `.unwrap()` panics —
    for the player, that kills the decode thread with no restart path. The
    pipeline modules handle poisoning correctly (`poisoned.into_inner()`,
    `mixing_layer.rs:554-557`); pick that policy (or `parking_lot`) everywhere.

15. **Use-after-free window in stream teardown** — `coreaudio_stream.rs:615-628`:
    the callback context is freed 50 ms after `AudioComponentInstanceDispose`.
    If disposal fails (it logs and continues), a still-registered callback could
    touch freed memory. The window is narrow but the ordering contract is
    "hope", not proof — tie the context's lifetime to confirmed unit disposal.

### Cleanup ordering that is *right* (keep)

- `file_player/source.rs:134-141` — the model thread lifecycle in the repo:
  atomic flag + stored handle + `Drop` that signals **and joins**.
- `now_playing/` — `timeout` + `kill_on_drop(true)` around `osascript`
  (`applescript.rs:113-132`), `Drop` aborts both tasks (`watcher.rs:125-129`).
- `devices/device_watcher.rs` — `Weak` upgrade guards, `Drop` aborts.
- Mixer removal ordering — producer detached *before* worker stop, with the
  reason documented (`mixing_layer.rs:289-293`, `pipeline_manager.rs:769-774`).
- `catch_unwind` at both `extern "C"` callback boundaries (unwinding across FFI
  is UB) — correctly done in both CoreAudio callbacks.

## Cross-cutting recommendations

1. Adopt the `file_player/source.rs` lifecycle (flag + handle + joining Drop) as
   the standard for every spawned loop; the broadcasting and recording tasks are
   the violators.
2. One CFString property helper; one policy for lock poisoning; per-context (not
   static) diagnostics in callbacks.
3. Idle loops must block on something (channel recv, `Notify`, interval) — the
   two 100 µs sleep-spinners burn a core doing nothing.
