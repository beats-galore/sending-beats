# Data Layer (db/, entities/, commands/, migrations)

**TL;DR — the sea-orm migration is finished (contrary to appearances), migration
discipline is genuinely excellent, and the keychain handling is exemplary. The
work remaining is deletion and hardening: two dead raw-sqlx files whose queries
are broken anyway, a documented convention in CLAUDE.md that prescribes a
runtime bug, zero transactions in the command layer, three queries that operate
across ALL configurations instead of the active one (one deletes user data),
and an FK landmine set to trigger the first time a custom effect is saved.**

## Decisions needed from Aaron

1. `commands/audio_effects.rs`: six registered commands silently succeed doing
   nothing (doc 08 §1) — reconnect or delete?
2. Three streaming stacks are registered; only `start_cast` is used by the UI —
   which survives?

## What this layer gets right

- **The sqlx→sea-orm migration is 100% done in practice.** Zero commands touch
  the sqlx pool; it exists only to run `sqlx::migrate!` (`db/mod.rs:62-75`).
- **Migration discipline**: all 22 tables use `VARCHAR(36)` UUID PKs and carry
  the required timestamp columns verbatim; zero DB-level enums or CHECK
  constraints — the project's own conventions, followed without exception.
- **Keychain for stream passwords** (`db/cast_secrets.rs`): keyed by row id so
  secret and row are forgotten together; empty-password-clears-entry;
  absent-is-not-an-error; non-macOS stub errors instead of silently dropping a
  credential; password never crosses IPC (`has_password` only).
- **Real DB tests**: 26 tests across bus/patch-layout/patch-color services run
  against in-memory DBs with the real migrations applied, named as behaviors
  (`a_removed_bus_takes_its_members_with_it`).
- **`commands/buses.rs` is the model command file** — `dispatch`/`persist`
  helpers, why-comments; `cast_configurations.rs` and `device_attachment.rs`
  (compensating rollback with a `created` flag) meet the same bar.
- **Uniform DB plumbing**: every command takes `State<AudioState>` →
  `.database.sea_orm()`; no globals or ad-hoc connections.

## High-severity findings

### H1. `db/recordings.rs` + `db/broadcasts.rs`: 666 dead lines, broken queries, and CLAUDE.md prescribes the bug

Nothing outside these files references their types (glob re-exports at
`db/mod.rs:22,31` suppress the dead-code lint). Their six runtime
`query_as::<_, T>` calls all use `SELECT id as "id: Uuid"` — but that
annotation is macro syntax (`query_as!`); at **runtime** SQLite returns a
column literally named `"id: Uuid"`, and `FromRow` fails with `ColumnNotFound`.
Every one of these functions errors on first call. **CLAUDE.md's "Type
Annotation Requirements" section mandates this pattern and must be corrected —
it currently causes the failure it claims to prevent.** Bonus:
`broadcast_configurations.password` is a plaintext `TEXT NOT NULL` column
(`migrations/20250925160006_broadcasts.sql:13`) that the newer keychain design
deliberately abolished. Delete both files, both re-exports, and consider
dropping the six orphaned tables.

### H2. Unscoped queries: removing a device destroys it in every saved configuration

- `commands/configurations.rs:479-483,510-514` — `remove_device_configuration`
  runs `delete_many` filtered on `DeviceIdentifier` **only**: removing a device
  from the current session also deletes it from every saved reusable
  configuration. Data loss.
- `configurations.rs:459-463` `get_device_channel_number` and
  `audio_devices.rs:229-236` — same unscoped lookup, arbitrary row wins when a
  device exists in two configs. (Four copies of this lookup exist; only
  `configurations.rs:320-328` scopes by `ConfigurationId` correctly.)

### H3. `audio_effects_custom` FK landmine

FKs **are** enforced (sqlx sets `foreign_keys = ON` by default; sea-orm
inherits it). `audio_effects_custom` references `configured_audio_devices`
(`migrations/20250925160003_audio_effects.sql:38`), but
`remove_device_configuration` deletes effects-default rows and device rows,
**never custom-effects rows**. Today the table is always empty (the feature is
disconnected — doc 08 §1), so deletes succeed. The first saved custom effect
makes **every subsequent device removal fail with an FK violation**. Same hole
in `migrations/20260814101507:27-33`. Also: the column is declared `JSONB`
(SQLite → NUMERIC affinity, will coerce numeric-looking strings) and nullable
in SQL but `String` (non-null) in the entity — a NULL row fails to deserialize.

### H4. `create_device_configuration`: race + orphaning

`configurations.rs:257-455` (199 lines): SELECT `max(channel_number)` then
INSERT with no transaction — the DeviceWatcher thread attaches devices
concurrently with user actions, so two devices can claim one channel. Device
row and effects row are inserted separately; failure between them leaves a
device with no effects row.

### H5. Zero transactions in `src/commands/`

No `.begin()` anywhere in the directory. Unprotected multi-write sequences:
`audio_devices.rs:245→288→302` (`safe_switch_input_device` — a failed DELETE is
*warned* then the INSERT proceeds, permanently losing the channel assignment),
`application_audio.rs:108→149`, `mixer.rs:53→91`, `file_player.rs:246→253`,
`file_player_service.rs:184-212` (`reorder_queue`: N unbatched
read-modify-writes). The transaction machinery exists and is used correctly
inside services (`seaorm_services.rs:79,134,290,419,751`,
`audio_bus_service.rs:89`) — push command sequences down into services.

## Medium findings

- **Stringly-typed boundary**: 130/132 commands return `Result<T, String>`;
  ~200 `map_err(|e| e.to_string())`. 18 commands use the success `String` as
  prose the frontend can't branch on. Introduce a serializable `CommandError`
  with `From<anyhow::Error>`.
- **`seaorm_services.rs` (824 lines, over the project's 800 limit)** contains
  the same "copy child rows to a new configuration" loop **nine times** across
  three session/reusable functions (`:115-538`). Every new column must be added
  in three places or it silently stops surviving a session copy. Extract
  `clone_children(txn, from, to, now)`.
- **Two pools, no WAL**: a 10-connection sqlx pool that only ever ran
  migrations + a 10-connection sea-orm pool (`db/mod.rs:62-66,121-128`);
  neither sets `journal_mode`, so SQLite runs in rollback-journal mode where a
  writer blocks readers. Migrate on a 1-connection pool, close it, and set
  `journal_mode=WAL` on the sea-orm options.
- **Service-layer bypass**: 20 inline sea-orm query sites in commands;
  `audio_application` and `mixer_channel` have no service at all;
  `configurations.rs:595-599` re-inlines a query its service already provides.
- **`lib.rs` registration**: 8 recording commands registered **twice**
  (`lib.rs:535-542` = `:577-584`); `check_screen_recording_permission` is a
  `#[tauri::command]` never registered; `commands/mod.rs:23-40` re-exports 18
  modules that `lib.rs` glob-imports directly anyway (dead, and violates the
  project's own no-re-export rule).
- **Engine-vs-DB write ordering is inconsistent**: the five effects-default
  commands update the audio engine first, then the DB (failure → engine and DB
  disagree, no rollback); `device_attachment.rs` does DB-first-with-rollback.
  Pick one policy.
- **`streaming.rs` clone-mutate-writeback** (`:46-74`): concurrent calls
  last-writer-win; also carries all 9 production `.unwrap()`s on a std Mutex —
  one panic poisons all 7 commands forever.
- **Entities use `id: String` while CLAUDE.md mandates `uuid::Uuid`** — all 15
  entities; service signatures split between `Uuid` and `&str`;
  `configurations.rs:560` parses a UUID it never uses.

## Migration-file findings

- **`20251008231700_remove_soft_deletes_and_audio_levels.sql` resurrects
  soft-deleted rows**: drops `deleted_at` (`:51-54`) without first deleting
  rows where it was set. Also drops `WHERE deleted_at IS NULL` partial indexes
  on the recording/broadcast tables and recreates them unpartialed while
  leaving those tables' `deleted_at` in place — half-finished.
- **`20260814101507` deletes user rows irrecoverably** with a `NOT EXISTS`
  guard keyed on a display name and `LIMIT 1` on duplicate names.
- **Non-rerunnable seeds**: `20251016101147` (40 rows, no `ON CONFLICT`),
  `20251016102658` (same); `20250925171710`'s `ON CONFLICT (id)` doesn't cover
  its partial unique index.
- **Missing indexes**: `file_players.breakpoint_track_id`,
  `configuration_cast_targets.cast_configuration_id` (the exact lookup its own
  CASCADE needs). **Missing FK**:
  `audio_mixer_configurations.reusable_configuration_id` — the one relationship
  left unconstrained.
- `system_audio_state` is a singleton by convention only, with the schema's
  only nullable boolean.

## Blocking calls on the async runtime (data-layer edition)

- `cast_configurations.rs:72,87`: blocking keychain `has_password()` once per
  row inside `list_cast_configurations`.
- `application_audio.rs:33,256,337,358`: sync ScreenCaptureKit FFI (up to 10 s)
  in `async fn`s.
- `file_player.rs:453,458`: sync `path.exists()` beside the same file's correct
  `tokio::fs::metadata` (`:184`) and `spawn_blocking` (`:203`).

## Cleanup list (low)

- Dead: `AudioDatabase::pool()`, `config_uuid` (`configurations.rs:560`),
  `check_via_tccutil` (always `Err`), `request_permissions` (always `Ok(true)`),
  placeholder commands `browse_audio_files`, `start_device_monitoring`; dead
  params `configuration_id` in 3 of 4 effects-default commands,
  `app_audio_state` (`application_audio.rs:246`).
- Unused imports: `PlayerEvent` (`file_player.rs:2`), `ApplicationAudioError`
  (`application_audio.rs:1`), `Set` (`configurations.rs:7`).
- Inline `use` inside fn bodies ×14 (nine in `icecast.rs` alone) — violates the
  project's imports-at-top rule.
- Naming drift: `list_/get_/enumerate_/refresh_` for collections;
  `delete_` vs `remove_`; three different names for "change a gain";
  `set_output_stream` adds rather than sets.
- Virtualness by name-matching: `device_id.contains("BlackHole")`
  (`device_attachment.rs:284`).
- Minor migration hygiene: lowercase `numeric` type, a redundant duplicate
  index on `audio_applications.bundle_identifier`.
