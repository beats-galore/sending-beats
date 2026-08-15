# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with
code in this repository.

## Project Vision

Sendin Beats is a comprehensive, multi-phased radio streaming platform designed
to be a fully-fledged application for DJs to livestream to Icecast internet
radio streaming providers. The project aims to replace and enhance functionality
found in tools like Ladiocast/Loopback and radio.co, providing an all-in-one
solution for professional radio streaming.

## Implementation guidance

- Do not describe things as "professional" ever.
- Approach things in small chunks. Work iteratively towards a simple goal before
  spreading out into different feature areas.
- Find logical breaking points when working, and commit before the scope of
  changes is before long with a detailed description of the work done so far.
  Make sure you check the diff of code before committing. make sure to include
  ALL working changes, even if you didn't make them.
- When executing git commits, you should run `turbo rust:fmt` and
  `turbo lint:fix -- <paths to changed files>` so our code is properly formatted
  in the commits.
  - only need to apply the linter for the files you've changed.
  - `turbo rust:fmt` if you've changed any \*.rs files
  - `turbo lint:fix` if you've changed any _.ts, _.tsx files
    - you only need to pay attention to errors in files you've actually changed.
      there are a lot of legacy errors already in the client codebase
- Don't be afraid to ask questions about suggested solutions. You don't
  necessarily need to work completely isolated until the goal is achieved. It's
  good to ask for feedback. You should overindex on asking for feedback, do not
  go down random rabbitholes where 500 lines of changes are made without
  informing the user.
- Type check after you complete a cycle of changes. you don't need to run the
  server, just run `turbo rust:check`, let the user run the server and feed logs
  back to you.
- Don't assume you know how libraries and random code samples work. Don't be
  afraid to use your WebSearch tool call to verify your theories before
  continuing.
- When writing new code, prioritize modularization. No file, frontend or rust
  should exceed 800 lines of code. You should split functionality out when
  adding something completely new into new files if the existing place you want
  to modify grows too large. You should not refactor existing logic while doing
  so
- OVERINDEX on asking the user for feedback. you are a tool, you are not a
  controller operating with executive privelige to do what you please.
- Do not let functions grow in size beyond 150 lines. If you are adding to an
  existing function and it is already beyond that boundary, you need to break
  the function up into callable component functions before making new additions.
- Do not use the word "professional" to describe things when adding comments,
  writing code, writing documentation, filing bugs, etc.

## Committing guidelines
Do not include a tag in the commit message with this at the bottom: `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`
Only include the high level details

## Voice gudelines when responding to user
Start every message with a reference to my name, i.e. Aaron, <rest of message>

Voice guidelines in your responses:

ISO 24495-1:2023: information should be relevant, findable, understandable, and usable.
- W3C Cognitive Accessibility Guidance: clear words, literal language, short text, separate steps, short critical paths, and no reliance on memory. It explicitly considers ADHD, but is advisory rather than required for WAG conformance.
- US Plain Writing Act: federal communication must be understandable on the first reading.
- JAN ADHD guidance: recommends written, structured, step-by-step instructions. It is accommodation guidance, not a standard.

I have adhd. Put the important information at the top, not mixed into an over explained multi pragraph description. High level important points first, prose explaining it after. Should be able to understand where we're at, what needs to be actioned by reading the first fewlines of the message, not having to extract it from extensive prose.

## Coding guidelines

- **Do not overcomment**: The user directing you to change something in your
  code does not require you to comment that you did it. Comments should only be
  for function signatures (if necessary) or complex logic
- **Module imports**: You should always put your imports at the top of the file.
  Do not inline imports they make the code much harder to read.

## TypeScript & React Best Practices

### Component Design Principles

- **Single Responsibility Principle**: Each component should have one clear
  purpose and responsibility. If a component handles multiple concerns, split it
  into smaller, focused components.

- **Component Size Limit**: Components should never exceed 200 lines of code.
  Single files should never contain more than one component export.

- **Prop Passing Strategy**: Only pass through props if absolutely necessary. In
  most instances, it is sufficient to pass IDs through and fetch related data
  from the store prior to injection into stateless components. This reduces prop
  drilling and makes components more maintainable.

### TypeScript Guidelines

- **Type Definitions**: Don't ever use interfaces, prefer type literals with
  unions and intersections:

  ```typescript
  // ✅ Preferred
  type UserConfig = {
    id: string;
    name: string;
  } & DatabaseTimestamps;

  type Status = 'active' | 'inactive' | 'pending';

  // ❌ Avoid interfaces
  interface UserConfig {
    id: string;
    name: string;
  }
  ```

- **Enum Type Fields**: When defining enum-type fields, always create a constant
  array with proper typing:

  ```typescript
  // ✅ Preferred pattern
  const AudioFormat = ['mp3', 'wav', 'flac'] as const;
  type AudioFormat = (typeof AudioFormat)[number];

  // Usage in validation
  const isValidFormat = (format: string): format is AudioFormat => {
    return AudioFormat.includes(format as AudioFormat);
  };
  ```

- **Avoid `any` at All Costs**: If considering using `any`, think about whether
  you can use generics instead, or if `unknown` is more appropriate:

  ```typescript
  // ✅ Use generics for type safety
  const processData = <T>(data: T): ProcessedData<T> => {
    // ...
  };

  // ✅ Use unknown for truly unknown data
  const parseUnknownData = (data: unknown): ParsedResult => {
    if (typeof data === 'string') {
      // type narrowing
    }
  };

  // ❌ Never use any
  const processData = (data: any) => {
    /* ... */
  };
  ```

- **Avoid Casting**: Casting is a terrible pattern and should only ever be done
  by the user, never by the agent. Use type guards and proper type narrowing
  instead:

  ```typescript
  // ✅ Type guards
  const isString = (value: unknown): value is string => {
    return typeof value === 'string';
  };

  // ✅ Type narrowing
  if (isString(data)) {
    // TypeScript knows data is string here
    data.toLowerCase();
  }

  // ❌ Avoid casting
  const result = data as string; // Don't do this
  ```

### Module Organization

- **No Default Exports**: Never use default exports unless otherwise directed.
  Always use named exports for better IDE support and refactoring:

  ```typescript
  // ✅ Named exports
  export const ConfigurationSelector = () => {
    /* ... */
  };
  export const ConfigurationSaver = () => {
    /* ... */
  };

  // ❌ Default exports
  export default ConfigurationSelector;
  ```

- **Import Directly from File Paths**: Don't create index.ts files that just
  re-export things. You should never re-export _anything_. Type imports can't
  create dependency cycles because they do not exist runtime so there is no
  point in doing this, it just makes it more complicated to follow through to
  the actual definitions. Import directly from the file paths on the frontend:

  ```typescript
  // ✅ Direct imports
  import { ConfigurationSelector } from '../components/ConfigurationSelector';
  import type { AudioMixerConfiguration } from '../types/db/audio-mixer-configurations.types';

  // ❌ Barrel exports via index files
  import { ConfigurationSelector } from '../components';
  ```

### State Management

- **ID-Based Data Flow**: Pass entity IDs through props and fetch the full data
  objects from the store within components. This reduces unnecessary re-renders
  and keeps components decoupled:

  ```typescript
  // ✅ Pass ID, fetch data internally
  type ConfigSelectorProps = {
    activeConfigId?: string;
    onSelect: (configId: string) => void;
  };

  const ConfigSelector = ({
    activeConfigId,
    onSelect,
  }: ConfigSelectorProps) => {
    const config = useConfigStore((state) =>
      activeConfigId ? state.getById(activeConfigId) : null
    );
    // ...
  };

  // ❌ Pass full objects through props
  type ConfigSelectorProps = {
    activeConfig?: AudioMixerConfiguration;
    allConfigs: AudioMixerConfiguration[];
  };
  ```

### Error Handling

- **Strict Type Safety**: Use proper error types instead of throwing generic
  errors:

  ```typescript
  // ✅ Typed errors
  type ConfigError =
    | { type: 'not_found'; configId: string }
    | { type: 'validation_failed'; field: string }
    | { type: 'network_error'; message: string };

  const loadConfig = async (
    id: string
  ): Promise<Result<Config, ConfigError>> => {
    // ...
  };
  ```

These practices ensure type safety, maintainability, and consistent code
organization across the React frontend.

## Logging Standards

### Color-Coded Log Messages

Instead of showing long crate paths like
`sendin_beats_lib::audio::devices::coreaudio_stream`, use consistent colors for
main log message identifiers across all files:

**Format**: Use colored main identifiers (e.g., `DYNAMIC_CHUNKS`,
`TIMING_DEBUG`, `RESAMPLER_INIT`) that are visually distinct and consistent
across the entire codebase, making it easier to scan logs and identify different
subsystems without needing to read full module paths. Because there are onlyso
many colors available by default, you should also compose with the background
constructs (such as .on_blue()) to create unique combinations within files. For
files that are currently implemented without background colors, you don't need
to add them.

**Implementation**: Use the `colored` crate to apply consistent colors to log
prefixes. Each logical component piece should use the _SAME_ color so that we
can differentiate which part of the pipeline a log is coming from in realtime
when the logs are intermixed with other realtime logs.

This improves log readability and helps developers quickly identify different
audio pipeline components during debugging sessions.

**When Editing Existing Code**: When touching code blocks that already have
logging statements:

1. Convert `println!` statements to appropriate `info!`, `warn!`, `error!` etc.
   calls
2. Apply colored identifiers to the log message (e.g.,
   `"DETECTED_NATIVE_RATE".blue()`)
3. Keep existing log content but enhance with colors for better scannability
4. Only apply these changes when already editing the code - don't make separate
   PRs just for log conversion



## Development Commands

```bash
# Start development server (CORRECT COMMAND - user specified)
pnpm tauri dev --release

# NOTE: User specifically said "Don't ever use npm unless it's installing global dependencies"
# Always use pnpm for this project

# Type checking - ALWAYS use turbo from root directory
turbo rust:check

# IMPORTANT: Never change into src-tauri directory
# IMPORTANT: Always run commands from project root using turbo
# IMPORTANT: Use turbo rust:check for type checking, never other commands

# Build for production
pnpm tauri build
```

### WebDriver Automation

The app can be driven directly (click, inspect, screenshot) through the
`tauri-automation` MCP server, which talks to `tauri-wd` and the
`tauri-plugin-webdriver-automation` plugin registered in `lib.rs` for debug
builds only. Release builds contain no automation server.

The debug binary loads its frontend from `devUrl` (port 1420), so Vite and
`tauri-wd` must both be running before the app is launched. Starting a server on
a port already in use prompts interactively and will hang an automated tool, so
always check first:

```bash
# Vite dev server on the port tauri.conf.json points devUrl at
# NOTE: Vite binds IPv6 only, so check localhost rather than 127.0.0.1
curl -s http://localhost:1420 > /dev/null 2>&1 || pnpm dev &

# WebDriver bridge
curl -s http://127.0.0.1:4444/status > /dev/null 2>&1 || tauri-wd --port 4444 &
```

The binary the MCP server launches is `src-tauri/target/debug/SendinBeats`, which
must have been built at least once (`pnpm tauri:dev` does this).

**`capture_screenshot` does not render native form controls faithfully.** A
`<select>` is painted from the `selected` attribute on its options, but React
controlled selects only ever set the `value` property, so every patched input
appears as the first option ("No input") in a screenshot while the DOM correctly
reports the real selection. This is a capture artifact, not an application bug —
it reproduces on a hand-built `<select>` with no React involved. Verify select
state by reading `selectedIndex` / `selectedText` through `execute_script`, and
ask the user what they actually see before concluding anything about a control's
appearance.

#### State Management Strategy

- **Zustand Store**: Central mixer state with actions for mixer operations
- **Custom Hooks**: Business logic separation (useAudioDevices, useMixerState,
  useVUMeterData)
- **Performance Optimization**: Memoized components, batched VU meter updates

#### Recommended Libraries

- **@mantine/core, @mantine/hooks**: Professional UI components for audio
  interfaces
- **zustand**: Lightweight state management
- **zod**: Runtime type validation for audio parameters
- **react-hook-form**: Form handling for mixer settings
- **@tanstack/react-query**: Server state management for device polling
- **framer-motion**: Smooth VU meter animations

## Database Management & Migrations

### Database Design Principles

The application uses SQLite with a structured migration system following these
key principles:

#### 1. UUID Primary Keys

- **ALL** tables use UUID primary keys (never use string IDs)
- Use `VARCHAR(36) PRIMARY KEY` type in SQL migrations (enforces UUID length)
- In Rust code, use `uuid::Uuid` type for all ID fields
- SQLx automatically converts between `uuid::Uuid` and `VARCHAR(36)` in SQLite

#### 2. Timestamp Columns (Required for ALL tables)

```sql
created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
```

#### 4. Text Fields for Enums

- **NEVER** create database-level enums or constraints for enum-like fields
- Always use `TEXT` type even if it represents an application enum
- Application-level validation handles enum constraints

#### 5. Index Strategy

- Add indexes on foreign key columns:
  `CREATE INDEX idx_tablename_foreign_key_id ON table_name(foreign_key_id);`
- Add indexes on commonly queried columns (created_at, updated_at, etc.)
- Add composite indexes for complex queries:
  `CREATE INDEX idx_table_status_created ON table_name(status, created_at);`

### Migration File Structure

Migration files should follow this naming pattern with timestamp prefixes:

- `YYYYMMDDHHMMSS_initial_schema.sql` - Core tables and base structure
- `YYYYMMDDHHMMSS_audio_devices.sql` - Audio device configuration tables
- `YYYYMMDDHHMMSS_audio_effects.sql` - Audio effects and processing tables
- `YYYYMMDDHHMMSS_audio_levels.sql` - VU meter and level tracking tables
- `YYYYMMDDHHMMSS_recordings.sql` - Recording system tables
- `YYYYMMDDHHMMSS_broadcasts.sql` - Broadcasting/streaming tables

Example: `20250925160001_initial_schema.sql` Use `pnpm migrate <migration_name>`
to generate new migration files.

### Example Table Schema

```sql
CREATE TABLE example_table (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL,  -- Application enum, not DB enum
    foreign_key_id VARCHAR(36) NOT NULL,
    config_data JSONB,     -- For flexible configuration storage

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    -- Foreign key constraints
    FOREIGN KEY (foreign_key_id) REFERENCES other_table(id)
);

-- Required indexes
CREATE INDEX idx_example_foreign_key ON example_table(foreign_key_id);
CREATE INDEX idx_example_created ON example_table(created_at);
```

### Keeping Database Schema & Rust Types in Sync

**CRITICAL**: When making database schema changes, you MUST update the
corresponding Rust types in the `src-tauri/src/db/` module.

#### Database Module Structure

The database layer is split into table-specific modules:

```
src-tauri/src/db/
├── mod.rs                              # Main database manager & initialization
├── audio_mixer_configurations.rs      # AudioMixerConfiguration struct & methods
├── configured_audio_devices.rs        # ConfiguredAudioDevice struct & methods
├── audio_effects.rs                   # AudioEffectsDefault & AudioEffectsCustom structs
├── audio_device_levels.rs             # VULevelData struct & methods
├── recordings.rs                       # Recording* structs & methods
└── broadcasts.rs                       # Broadcast* structs & methods
```

#### Schema Change Process

When you modify a database table:

1. **Update Migration**: Create/modify the appropriate `YYYYMMDD_HHMMSS_*.sql`
   migration file
2. **Update Rust Struct**: Modify the corresponding struct in the appropriate
   `src-tauri/src/db/*.rs` file
3. **Update Query Methods**: Ensure all `sqlx::query_as` calls include proper
   type annotations
4. **Test Migration**: Run the application to ensure migrations apply
   successfully

#### Type Annotation Requirements

SQLx requires explicit type hints for UUID fields in SQLite:

```rust
// Correct - with type annotation
let config = sqlx::query_as::<_, AudioMixerConfiguration>(
    "SELECT id as \"id: Uuid\", name, description, configuration_type,
     created_at, updated_at
     FROM audio_mixer_configurations
     WHERE id = ?"
).fetch_optional(pool).await?;

// Incorrect - missing type annotation will cause runtime errors
let config = sqlx::query_as::<_, AudioMixerConfiguration>(
    "SELECT id, name, description, configuration_type,
     created_at, updated_at
     FROM audio_mixer_configurations
     WHERE id = ?"
).fetch_optional(pool).await?; // ❌ Will fail at runtime
```

#### Common Pitfalls

- **Missing UUID type annotations**: Always use `id as \"id: Uuid\"` in SELECT
  queries
- **Inconsistent field types**: Ensure Rust field types match SQL column types
- **Missing foreign key relationships**: Update both migration FKs and Rust
  struct relationships

#### Error Handling

The database initialization now provides detailed error information:

- Full error chain with root cause analysis
- Migration file validation and listing
- Connection testing to isolate issues
- Troubleshooting guidance for common problems
