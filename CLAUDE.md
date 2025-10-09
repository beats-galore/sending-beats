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
  `turbo lint:fix` so our code is properly formatted in the commits.
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

## Current Implementation Status

**Phase**: Early development - Virtual mixer UI implementation with backend
infrastructure **Architecture**: Tauri (Rust backend) + React TypeScript
frontend **Current Features**: Professional virtual mixer interface, audio
device enumeration, streaming client foundation

## MAJOR BREAKTHROUGH: Audio Engine is Working! 🎉

### ✅ **AUDIO SYSTEM FULLY FUNCTIONAL** (Just Completed)

- **Real Audio Capture**: Live audio input from microphones, virtual devices,
  system audio
- **Real Audio Output**: Sound playing through speakers, headphones, virtual
  outputs
- **Professional Audio Processing**: Live effects chain (EQ, compressor,
  limiter)
- **Real-time VU Meters**: Actual audio levels from captured samples
- **Hardware-Synchronized Timing**: No more timing drift, callback-driven
  processing

### What's Currently Working

- ✅ **REAL AUDIO SYSTEM**: Live capture, processing, and output working
- ✅ **Professional Audio Pipeline**: Input → EQ → Compressor → Limiter → Master
  → Output
- ✅ **Hardware Synchronization**: Callback-driven processing eliminates timing
  drift
- ✅ **Real-time VU Meters**: Displaying actual audio levels from live
  processing
- ✅ **Multiple Audio Devices**: Support for BlackHole, system audio,
  microphones, speakers
- ✅ **Low-latency Processing**: Hardware-aligned buffer sizes for optimal
  performance
- ✅ **Professional Effects**: Working 3-band EQ, compressor, and limiter
- ✅ **Virtual mixer UI**: Horizontal layout with real audio controls

### Remaining Tasks (Minor Refinements)

- 🔧 **Audio Effects Chain**: Test effects parameters and ensure artifacts-free
  processing
- 🔧 **Stereo Channel Mixing**: Verify L/R channel separation and mixing
  accuracy
- 🔧 **Performance Optimization**: Fine-tune buffer sizes and CPU usage
- 🔧 **Error Handling**: Robust recovery from device disconnections
- 🔧 **UI Polish**: Connect all mixer controls to real audio parameters

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

### Technical Implementation Notes

- User has BlackHole 2CH, microphone, MacBook speakers, and BenQ monitor
  available
- Focus on macOS Core Audio implementation first
- Use cpal for cross-platform audio stream management
- Audio processing should happen in separate thread from UI
- Maintain horizontal layout that user requested

## Known Working Components

- Device enumeration and filtering works correctly
- UI polling and updates work at 10 FPS (100ms intervals)
- Professional mixer interface is complete and responsive
- Backend-frontend communication is solid
- Effects parameter structures are implemented

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
