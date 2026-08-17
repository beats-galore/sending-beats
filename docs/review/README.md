# Backend Review — Sweet Beats Studio

*Full-depth review of `src-tauri/` (~36,600 lines of Rust, 142 files, 132 Tauri
commands), August 16, 2026. Working tree state including the in-progress
file_player split.*

## The one-paragraph verdict

The current audio pipeline generation — deadline-scheduled worker threads, SPSC
rings, an output-paced mixer, Little's-Law latency accounting — is the right
architecture, and the newest code (pacing, block accumulator, bus mixer, file
player, now-playing) is genuinely well-crafted Rust. The dominant problem is
that **the previous generations were never buried**: dead pipeline skeletons are
still constructed and still *called* (one makes every input-device removal
error), three streaming stacks are registered at once and the one the UI
doesn't call is non-functional, the effects surface silently no-ops at three
layers, and ~400 clippy warnings are invisible because the standard check runs
with `-A warnings`. The realtime inner loops undermine the sound architecture
with per-sample atomics, per-cycle allocations, RT-thread logging, and
`static mut` diagnostics that are UB with two devices. Almost everything found
is deletable or locally fixable — this is a pruning-and-hardening review, not a
rewrite review.

## Read in this order

| Doc | Vector | Headline |
|---|---|---|
| [08](08-silently-broken-features.md) | **Silently broken features** *(added vector)* | Effects knobs, broadcasting, solo, MP3 finalize — success reported, nothing happens |
| [02](02-dead-code-and-deprecation.md) | Dead code & deprecation | The inventory: dead generations, dead modules, dead symbols, unused deps |
| [01](01-architecture-and-abstractions.md) | Architecture & abstractions | Right architecture; wrong-shaped worker trait, swapped rate fields, no single owner for channel state |
| [03](03-performance-and-realtime.md) | Performance & realtime | Per-sample atomics, RT allocations/logging, coordinator stalls, drift-controller doubts |
| [04](04-memory-and-resource-safety.md) | Memory & resources | `static mut` UB in callbacks, FFI leaks, unkillable 10 kHz tasks |
| [05](05-code-quality-and-rust-conformance.md) | Code quality & conventions | The repo's own rules, scored; `-A warnings` hides everything |
| [06](06-data-layer.md) | Data layer | sea-orm migration is done; unscoped deletes, zero transactions, FK landmine |
| [07](07-events-vs-polling.md) | Events vs. polling *(requested vector)* | Six poll loops; push infra already exists for all of them |

## What the repo gets right

Worth stating plainly, because the strengths are load-bearing for every
recommendation — the fixes below are "make the rest of the code like the best
of it":

- **The pipeline architecture itself.** Hardware callbacks touch only ring
  buffers; every stage between them runs on its own thread with a mach
  time-constraint deadline (`realtime_thread.rs` — correctly implemented,
  including advisory refusal). Pacing by downstream *occupancy* rather than
  ring capacity is the key latency decision, made correctly and documented.
- **`block_accumulator.rs`** — the window-floor shedding design distinguishes
  bursts from standing backlog and sheds only the latter. This is what keeps
  long-session latency flat, and its tests encode the reasoning.
- **`bus_mixer.rs`** — one sum per bus regardless of destination count, with
  behavior-named tests against a real ring-buffer harness.
- **`latency_probe.rs`** — time-weighted occupancy gauges (Little's Law),
  single-writer atomics, zero locks on the audio path. The model for all
  future realtime observability.
- **`file_player/source.rs`** — the model thread lifecycle (flag + handle +
  joining `Drop`); `now_playing/` — the model service module (`thiserror`,
  timeouts, `kill_on_drop`, fixture tests); `devices/device_watcher.rs` —
  burst coalescing with `Weak` guards.
- **`effects/` DSP** — allocation-free process paths, denormal/non-finite
  flushing.
- **Data-layer discipline** — all 22 tables follow the UUID/timestamps/TEXT-enum
  conventions without exception; keychain handling for stream passwords is
  exemplary; 26 service tests run against real migrations.
- **The comment culture of the newest code** — comments explain *why* and
  invariants live in tests. You can date any file in this repo by whether its
  comments explain why (current) or narrate changelogs in `**BOLD MARKERS**`
  (older strata) — the gradient itself is a useful review tool.

## The five decisions that unlock everything else

1. **Bury the dead generations** (doc 02): `PipelineQueues`, `VirtualMixer`,
   `tap/`, two of three streaming stacks, `db/recordings.rs`+`broadcasts.rs`,
   the vestigial coordinator ledger. Two removal bugs and most of the warning
   noise disappear with them.
2. **Decide the fate of the effects surface** (doc 08 §1): reconnect it
   end-to-end or delete it honestly. It is currently lying to the UI.
3. **Decide which streaming stack survives** (doc 08 §2) — the UI calls
   `start_cast`; the other two include a non-functional broadcast path.
4. **Turn warnings back on** (`-A warnings` → off, clippy in the gate) and let
   the compiler keep the graveyard from refilling.
5. **Adopt the repo's own best patterns as law**: `source.rs` lifecycle for
   every spawned loop, `latency_probe`-style counters instead of RT logging,
   `buses.rs`-style command helpers, `channel_placement_for`-style shared
   lookups.

## Statistics

| | |
|---|---|
| Rust LOC (src-tauri/src) | ~36,600 across 142 files |
| Tauri commands registered | 132 (8 registered twice; 1 written but never registered) |
| Commands never invoked by the frontend | 35 |
| Frontend invokes with no backend command | 5 (throw at runtime) |
| Zombie commands (invoked, structurally can't work) | 10+ (effects ×6, listener stats, recording metadata, tap family) |
| Unused Cargo dependencies | 14 now, 18 after deletions (`.ogg` decode likely broken — see doc 02) |
| Clippy warnings (suppressed in dev loop) | ~400 |
| Files over the repo's 800-line limit | 9 |
| Functions over the 150-line limit | 5 |
| Largest file | `devices/coreaudio_stream.rs` (1,647) |
| Streaming implementations | 3 (1 live, 1 non-functional, 1 half-dead) |
| MP3 encoders | 2 (1 real, 1 returns PCM labeled as MP3) |
| DB stacks | 2 (sea-orm live everywhere; sqlx = migrations only + 666 dead lines) |

*Review method: the realtime core (mixer pipeline, stream management,
coreaudio_stream, resampling, drift control) was reviewed line-by-line in the
main session; three parallel reviewers covered dead-code cross-referencing,
the data/command layer, and the service modules; every load-bearing claim was
re-verified against the source before inclusion. File:line references
throughout are to the working tree at review time.*
