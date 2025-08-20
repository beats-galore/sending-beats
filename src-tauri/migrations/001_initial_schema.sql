-- Initial database schema for Sendin Beats audio mixer
-- Version: 001
-- Purpose: Core audio level buffering and configuration storage

-- VU Meter levels ring buffer with automatic cleanup
-- This table stores real-time audio level data with high throughput
CREATE TABLE vu_levels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,          -- Unix timestamp in microseconds for precision
    channel_id INTEGER NOT NULL,         -- Channel identifier
    peak_left REAL NOT NULL DEFAULT 0.0, -- Left channel peak level (-inf to 0 dB)
    rms_left REAL NOT NULL DEFAULT 0.0,  -- Left channel RMS level (-inf to 0 dB)
    peak_right REAL DEFAULT 0.0,         -- Right channel peak level (NULL for mono)
    rms_right REAL DEFAULT 0.0,          -- Right channel RMS level (NULL for mono)
    is_stereo BOOLEAN NOT NULL DEFAULT FALSE, -- Whether this is stereo data
    UNIQUE(timestamp, channel_id) ON CONFLICT REPLACE
);

-- Optimized indexes for real-time queries
CREATE INDEX idx_vu_timestamp ON vu_levels(timestamp DESC);
CREATE INDEX idx_vu_channel_timestamp ON vu_levels(channel_id, timestamp DESC);

-- Master output levels (separate table for master bus)
CREATE TABLE master_levels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    peak_left REAL NOT NULL DEFAULT 0.0,
    rms_left REAL NOT NULL DEFAULT 0.0,
    peak_right REAL NOT NULL DEFAULT 0.0,
    rms_right REAL NOT NULL DEFAULT 0.0,
    UNIQUE(timestamp) ON CONFLICT REPLACE
);

CREATE INDEX idx_master_timestamp ON master_levels(timestamp DESC);

-- Audio device configuration and state
CREATE TABLE audio_devices (
    id TEXT PRIMARY KEY,                  -- Device identifier (input_device_id/output_device_id)
    name TEXT NOT NULL,                   -- Human-readable device name
    device_type TEXT NOT NULL CHECK(device_type IN ('input', 'output')), -- Device type
    sample_rate INTEGER NOT NULL,        -- Native sample rate
    channels INTEGER NOT NULL,           -- Number of channels
    is_default BOOLEAN NOT NULL DEFAULT FALSE, -- Whether this is the default device
    is_active BOOLEAN NOT NULL DEFAULT FALSE,  -- Whether this device is currently active
    last_seen INTEGER NOT NULL,          -- Last time this device was detected
    created_at INTEGER NOT NULL DEFAULT (unixepoch('now')),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch('now'))
);

CREATE INDEX idx_devices_type ON audio_devices(device_type);
CREATE INDEX idx_devices_active ON audio_devices(is_active);

-- Channel configuration and settings
CREATE TABLE channels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,                   -- Channel name (e.g., "Deck A", "Microphone")
    input_device_id TEXT,                 -- Reference to audio_devices.id
    gain REAL NOT NULL DEFAULT 1.0,      -- Channel gain (linear, 0.0 to 2.0+)
    pan REAL NOT NULL DEFAULT 0.0,       -- Pan position (-1.0 = left, 0.0 = center, 1.0 = right)
    muted BOOLEAN NOT NULL DEFAULT FALSE, -- Mute state
    solo BOOLEAN NOT NULL DEFAULT FALSE, -- Solo state
    effects_enabled BOOLEAN NOT NULL DEFAULT FALSE, -- Whether effects are enabled
    
    -- EQ settings
    eq_low_gain REAL NOT NULL DEFAULT 0.0,   -- Low frequency gain in dB (-12 to +12)
    eq_mid_gain REAL NOT NULL DEFAULT 0.0,   -- Mid frequency gain in dB (-12 to +12)
    eq_high_gain REAL NOT NULL DEFAULT 0.0,  -- High frequency gain in dB (-12 to +12)
    
    -- Compressor settings
    comp_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    comp_threshold REAL NOT NULL DEFAULT -12.0,  -- Compression threshold in dB
    comp_ratio REAL NOT NULL DEFAULT 4.0,        -- Compression ratio
    comp_attack REAL NOT NULL DEFAULT 5.0,       -- Attack time in ms
    comp_release REAL NOT NULL DEFAULT 100.0,    -- Release time in ms
    
    -- Limiter settings
    limiter_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    limiter_threshold REAL NOT NULL DEFAULT -0.1, -- Limiter threshold in dB
    
    created_at INTEGER NOT NULL DEFAULT (unixepoch('now')),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch('now')),
    
    FOREIGN KEY (input_device_id) REFERENCES audio_devices(id)
);

CREATE INDEX idx_channels_device ON channels(input_device_id);
CREATE INDEX idx_channels_active ON channels(muted, solo);

-- Output routing configuration
CREATE TABLE output_routes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,                   -- Route name (e.g., "Main", "Monitor", "Stream")
    output_device_id TEXT NOT NULL,      -- Reference to audio_devices.id
    gain REAL NOT NULL DEFAULT 1.0,      -- Output gain
    enabled BOOLEAN NOT NULL DEFAULT TRUE, -- Whether this output is active
    is_master BOOLEAN NOT NULL DEFAULT FALSE, -- Whether this is the master output
    created_at INTEGER NOT NULL DEFAULT (unixepoch('now')),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch('now')),
    
    FOREIGN KEY (output_device_id) REFERENCES audio_devices(id)
);

CREATE INDEX idx_output_routes_device ON output_routes(output_device_id);
CREATE INDEX idx_output_routes_master ON output_routes(is_master);

-- Mixer global settings
CREATE TABLE mixer_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch('now'))
);

-- Insert default settings
INSERT INTO mixer_settings (key, value) VALUES 
    ('sample_rate', '48000'),
    ('buffer_size', '256'),
    ('master_gain', '1.0'),
    ('auto_start', 'true'),
    ('vu_retention_seconds', '60');