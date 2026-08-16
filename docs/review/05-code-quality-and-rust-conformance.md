# Code Quality & Rust Conformance

**TL;DR — the newest code (file_player, now_playing, bus_mixer, pacing,
block_accumulator) is genuinely good Rust with behavior-named tests and
why-comments. The older strata violate the project's own rules — size limits,
logging standards, no-re-export, imports-at-top — and ~400 clippy warnings are
invisible because the standard check runs with `-A warnings`.** Quality here is
an archaeology gradient, not a uniform property: you can date a file by whether
its comments explain *why*.

## Make the tools tell you this (first move)

- `turbo rust:check` runs `RUSTFLAGS="-A warnings" cargo check` — every warning
  suppressed in the standard loop. `cargo clippy` reports ~400 warnings:
  ~130 unused imports/variables (mostly dead-code markers), 14 `static mut`
  refs (UB class — doc 04 §1), 26 manual reimplementations of std patterns,
  6 clamp-patterns, 4+ too-many-arguments, deprecated `libc` items,
  `unreachable_patterns` on the double-registered commands.
- Recommendation: remove `-A warnings`; add `rust:clippy` to the pre-commit
  gate; drive to zero via the dead-code deletions in doc 02 (most warnings
  vanish with the deletions rather than needing individual fixes).

## Project-rule violations (the project's own standards)

### File size (≤800 lines)

| File | Lines |
|---|---|
| `devices/coreaudio_stream.rs` | 1647 |
| `stream_management/isolated_audio_manager.rs` | 1558 |
| `file_player/player.rs` | 1032 |
| `pipeline/pipeline_manager.rs` | 946 |
| `recording/types.rs` | 933 |
| `pipeline/mixing_layer.rs` | 851 |
| `recording/recording_writer.rs` | 825 |
| `db/seaorm_services.rs` | 824 |
| `pipeline/audio_worker.rs` | 820 |

Natural seams already exist: `coreaudio_stream` → property helpers / output /
input / callbacks; `player.rs` → `decode.rs` (the queue and metadata splits
already happened); `recording/types.rs` → extract the two preset factories;
`seaorm_services.rs` → extract the thrice-duplicated child-row copier.

### Function size (≤150 lines)

- `lib.rs::run()` — **462**
- `commands/configurations.rs::create_device_configuration` — 199
- `broadcasting/bridge.rs::run` — 191 (also contains the deadlock + command-eating bug)
- `commands/audio_devices.rs::safe_switch_output_device` — 155
- `mixing_layer.rs::start`'s worker closure — ~380 lines inline; the command
  handler and the collection/sync/mix steps are extractable functions.

### Logging standards (own rule: tracing + colored identifiers)

- Raw `println!`/`eprintln!` counts: `broadcasting/bridge.rs` 18,
  `file_player/player.rs` 13, `queue_types.rs` 4, `stream_manager.rs` 6,
  `file_player/manager.rs` 3, plus scattered singles.
- Beyond the standard: `info!` on realtime threads is a latency hazard
  (doc 03 §3) — the logging standard needs one more sentence: *never on audio
  threads; counters + reporter instead*.

### No-re-export rule (frontend rule, but the backend proves its point)

`audio/mod.rs:26-70` bulk re-exports ~45 symbols including entire dead families;
`broadcasting/service.rs` is a 15-line file that only re-exports its siblings
(so `commands/icecast.rs` imports a function from a module it doesn't live in);
`commands/mod.rs:23-40` re-exports 18 modules nothing imports through. The glob
re-exports are also what mute `dead_code` warnings for unused types.

### Imports-at-top rule

14 inline `use` statements inside function bodies (`commands/icecast.rs` ×9,
`db/cast_configuration_service.rs` ×3, `commands/application_audio.rs` ×2 —
one duplicated).

## Rust idiom findings

- **Error handling is three dialects**: `thiserror` domain errors (now_playing,
  BusError — good), `anyhow` (pipeline/coordinator — fine), raw `String`
  (`rubato.rs`, all 132 commands, `queue_types.rs`). Standardize: `thiserror`
  in domain modules, `anyhow` at composition, serializable `CommandError` at
  the IPC boundary.
- **Lock poisoning policy is inconsistent**: pipeline modules recover
  (`poisoned.into_inner()`); `file_player/player.rs` has ~60 `.lock().unwrap()`
  and `broadcasting/streaming.rs` 10 — one panic while held becomes permanent
  failure of the subsystem (doc 04 §14).
- **`try_lock`-and-silently-skip** ×6 in `tap/virtual_stream.rs`, plus
  `recording_service.rs:338-344` (an *async* fn that could `.lock().await`
  instead returns empty on contention) — contention becomes invisible data loss;
  at minimum count it.
- **Getter/setter culture**: `AudioWorkerState` exposes 13 trivial accessors
  consumed only by the trait plumbing (doc 01 §A1); public fields or direct
  struct access are idiomatic here.
- **Stats structs report constants** (`is_running: true`, counters never
  incremented) — doc 08 §11; either wire or delete.
- **`unsafe` quality is mixed but mostly justified**: `catch_unwind` at FFI
  callbacks (correct and necessary); `unsafe impl Send for Mp3Encoder` is
  defensible but its SAFETY comment states a conclusion, not the argument;
  the `static mut` family is the real problem (doc 04 §1); FFI length/layout
  issues in doc 04 §2-3. `mach_thread_self()` leaks a port right per thread
  (`realtime_thread.rs:71`) — cosmetic.
- **Debug-derives leak secrets**: `StreamingServiceConfig`,
  `IcecastSourceClient`, `StreamConfig` all derive `Debug` with plaintext
  `password` fields — one `{:?}` log line writes the Icecast password to disk.
  Manual `Debug` with redaction.
- **Copy-paste evolution markers**: comments like `**PERFORMANCE FIX**`,
  `**CRITICAL FIX**`, `**NEW ARCHITECTURE**` label code that has since been
  stubbed out or superseded — two of the "PERFORMANCE FIX" buffers end in
  `.clone()` (doc 03 §1). These annotations age into misinformation; the
  newer files' style (explain the invariant, not the changelog) is the model.
- **Magic numbers without names**: 150 ms/50 ms teardown sleeps, 96000 buffer
  caps, `chunk_size * 16` ring sizes, 0.95/0.85 limiter thresholds, PI gains
  `kp/ki` marked "(tune!)" — name them and say why, as `OUTPUT_TARGET_BLOCKS`
  and `MAX_BACKLOG_SAMPLES` already do.

## Testing

- **Where tests exist, they are excellent**: bus_mixer (9), block_accumulator
  (6), pacing, queue_manager, realtime_thread, file_player/queue, now_playing
  fixtures, 26 DB service tests against real migrations. Test names read as
  behavior sentences — keep this convention.
- **Where they're missing is exactly where bugs were found**: pipeline_manager
  lifecycle (removal bugs, doc 08 §4), solo aggregation, drift-controller
  direction (doc 03 §7), coreaudio_stream callbacks (hard to test, but the
  pure parts — chunking math, drain logic — are extractable), every command
  handler. The lifecycle bugs in this review would all have been caught by a
  "add two devices, remove two devices" integration test around `AudioPipeline`.

## Conformance quick-reference

| Project rule | Status |
|---|---|
| Files ≤800 lines | 9 violations (table above) |
| Functions ≤150 lines | 5 violations |
| tracing + colored identifiers | ~45 `println!` remain; RT-thread logging unaddressed |
| No re-exports | Violated across audio/, broadcasting/, commands/ |
| Imports at top | 14 inline `use` |
| UUID types for IDs | Entities all use `String` (and CLAUDE.md's sqlx guidance is itself wrong — doc 06 H1) |
| DB conventions (PKs, timestamps, TEXT enums) | **Fully followed** — 22/22 tables |
