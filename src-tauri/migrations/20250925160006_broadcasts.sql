-- Broadcasting/streaming system tables
-- Manages broadcast configurations and streaming sessions

-- Broadcast configurations - streaming server settings and quality presets
CREATE TABLE broadcast_configurations (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,

    -- Server connection details
    server_url TEXT NOT NULL,
    mount_point TEXT NOT NULL,
    username TEXT NOT NULL,
    password TEXT NOT NULL,                   -- Will be encrypted in application

    -- Audio quality settings
    bitrate INTEGER NOT NULL,                 -- Target bitrate in kbps
    sample_rate INTEGER NOT NULL,
    channel_format TEXT NOT NULL,            -- 'stereo' or 'mono'
    codec TEXT NOT NULL,                     -- 'mp3', 'aac', 'ogg'
    is_variable_bitrate BOOLEAN DEFAULT FALSE,
    vbr_quality INTEGER,                     -- VBR quality 0-9 (if VBR enabled)

    -- Stream metadata
    stream_name TEXT,
    stream_description TEXT,
    stream_genre TEXT,
    stream_url TEXT,                         -- Homepage URL for the stream

    -- Connection settings
    should_auto_reconnect BOOLEAN DEFAULT TRUE,
    max_reconnect_attempts INTEGER DEFAULT 10,
    reconnect_delay_seconds INTEGER DEFAULT 5,
    connection_timeout_seconds INTEGER DEFAULT 30,

    -- Quality monitoring
    buffer_size_ms INTEGER DEFAULT 500,
    enable_quality_monitoring BOOLEAN DEFAULT TRUE,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE NULL
);

-- Broadcasts - active and historical streaming sessions
CREATE TABLE broadcasts (
    id VARCHAR(36) PRIMARY KEY,
    broadcast_config_id VARCHAR(36),               -- May be null if config was deleted

    -- Session information
    session_name TEXT,
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    end_time TIMESTAMP WITH TIME ZONE,     -- Null if still active
    duration_seconds REAL,                  -- Total duration (calculated on end)

    -- Connection details (snapshot from config at start time)
    server_url TEXT NOT NULL,
    mount_point TEXT NOT NULL,
    stream_name TEXT,

    -- Audio format (snapshot from config at start time)
    bitrate INTEGER NOT NULL,
    sample_rate INTEGER NOT NULL,
    channel_format TEXT NOT NULL,
    codec TEXT NOT NULL,
    actual_bitrate REAL,                    -- Measured average bitrate

    -- Connection statistics
    bytes_sent BIGINT DEFAULT 0,
    packets_sent BIGINT DEFAULT 0,
    connection_uptime_seconds BIGINT DEFAULT 0,
    reconnect_count INTEGER DEFAULT 0,

    -- Quality metrics
    average_bitrate_kbps REAL,
    packet_loss_rate REAL DEFAULT 0.0,
    latency_ms INTEGER,
    buffer_underruns INTEGER DEFAULT 0,
    encoding_errors INTEGER DEFAULT 0,

    -- Status tracking
    final_status TEXT,                      -- 'completed', 'disconnected', 'error', 'cancelled'
    last_error TEXT,                        -- Last error message if any

    -- Listener statistics (if available from server)
    peak_listeners INTEGER,
    total_listener_minutes REAL,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE NULL,

    -- Foreign key constraints
    FOREIGN KEY (broadcast_config_id) REFERENCES broadcast_configurations(id)
);

-- Broadcast output - streaming data chunks for analysis/debugging
CREATE TABLE broadcast_output (
    id VARCHAR(36) PRIMARY KEY,
    broadcast_id VARCHAR(36) NOT NULL,
    chunk_sequence BIGINT NOT NULL,          -- Sequence number for ordering
    chunk_timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    chunk_size_bytes INTEGER NOT NULL,
    encoding_duration_ms REAL,              -- Time taken to encode this chunk
    transmission_duration_ms REAL,          -- Time taken to transmit this chunk

    -- Optional: store actual audio data for analysis (use sparingly due to size)
    audio_data BLOB,                        -- Only store for debugging/analysis

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE NULL,

    -- Foreign key constraints
    FOREIGN KEY (broadcast_id) REFERENCES broadcasts(id)
);

-- Indexes for broadcast_configurations
CREATE INDEX idx_broadcast_configurations_created ON broadcast_configurations(created_at);
CREATE INDEX idx_broadcast_configurations_active ON broadcast_configurations(deleted_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_broadcast_configurations_name ON broadcast_configurations(name) WHERE deleted_at IS NULL;
CREATE INDEX idx_broadcast_configurations_server ON broadcast_configurations(server_url) WHERE deleted_at IS NULL;

-- Indexes for broadcasts
CREATE INDEX idx_broadcasts_config_id ON broadcasts(broadcast_config_id);
CREATE INDEX idx_broadcasts_start_time ON broadcasts(start_time);
CREATE INDEX idx_broadcasts_created ON broadcasts(created_at);
CREATE INDEX idx_broadcasts_active ON broadcasts(deleted_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_broadcasts_status ON broadcasts(final_status) WHERE deleted_at IS NULL;
CREATE INDEX idx_broadcasts_duration ON broadcasts(duration_seconds) WHERE deleted_at IS NULL;

-- Composite index for active sessions
CREATE INDEX idx_broadcasts_active_sessions ON broadcasts(start_time, end_time) WHERE deleted_at IS NULL AND end_time IS NULL;

-- Indexes for broadcast_output
CREATE INDEX idx_broadcast_output_broadcast_id ON broadcast_output(broadcast_id);
CREATE INDEX idx_broadcast_output_sequence ON broadcast_output(broadcast_id, chunk_sequence) WHERE deleted_at IS NULL;
CREATE INDEX idx_broadcast_output_timestamp ON broadcast_output(chunk_timestamp);
CREATE INDEX idx_broadcast_output_created ON broadcast_output(created_at);
CREATE INDEX idx_broadcast_output_active ON broadcast_output(deleted_at) WHERE deleted_at IS NULL;