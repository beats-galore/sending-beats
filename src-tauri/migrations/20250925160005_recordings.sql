-- Recording system tables
-- Manages recording configurations, active recordings, and output storage

-- Recording configurations - templates for recording settings
CREATE TABLE recording_configurations (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    directory TEXT NOT NULL,
    format TEXT NOT NULL,                      -- 'mp3', 'wav', 'flac'
    sample_rate INTEGER NOT NULL,
    bitrate INTEGER,                           -- Nullable for lossless formats like WAV
    filename_template TEXT NOT NULL,
    default_title TEXT,
    default_album TEXT,
    default_genre TEXT,
    default_artist TEXT,
    default_artwork TEXT,                      -- Path to default artwork file
    auto_stop_on_silence BOOLEAN DEFAULT FALSE,
    silence_threshold_db REAL,                 -- Threshold in dB for silence detection
    max_file_size_mb INTEGER,                  -- Maximum file size before splitting
    split_on_interval_minutes INTEGER,         -- Split recording every N minutes
    channel_format TEXT NOT NULL,             -- 'stereo' or 'mono'
    bit_depth INTEGER NOT NULL,               -- 16, 24, or 32

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE NULL
);

-- Recordings - actual recorded files with metadata
CREATE TABLE recordings (
    id VARCHAR(36) PRIMARY KEY,
    recording_config_id VARCHAR(36),                 -- May be null if config was deleted

    -- File information
    internal_directory TEXT NOT NULL,         -- Internal storage path
    file_name TEXT NOT NULL,
    size_mb REAL NOT NULL,

    -- Audio format details
    format TEXT NOT NULL,                     -- 'mp3', 'wav', 'flac'
    sample_rate INTEGER NOT NULL,
    bitrate INTEGER,                          -- Nullable for lossless formats
    duration_seconds REAL NOT NULL,
    channel_format TEXT NOT NULL,            -- 'stereo' or 'mono'
    bit_depth INTEGER NOT NULL,

    -- Metadata
    title TEXT,
    album TEXT,
    genre TEXT,
    artist TEXT,
    artwork TEXT,                             -- Path to artwork file
    album_artist TEXT,
    composer TEXT,
    track_number INTEGER,
    total_tracks INTEGER,
    disc_number INTEGER,
    total_discs INTEGER,
    copyright TEXT,
    bpm INTEGER,
    isrc TEXT,                                -- International Standard Recording Code
    encoder TEXT,
    encoding_date TIMESTAMP WITH TIME ZONE,
    comment TEXT,
    year INTEGER,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE NULL,

    -- Foreign key constraints
    FOREIGN KEY (recording_config_id) REFERENCES recording_configurations(id)
);

-- Recording output - binary audio data storage (many-to-one with recordings)
CREATE TABLE recording_output (
    id VARCHAR(36) PRIMARY KEY,
    recording_id VARCHAR(36) NOT NULL,
    chunk_sequence INTEGER NOT NULL,         -- For splitting large recordings
    output_data BLOB NOT NULL,               -- Binary audio data

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE NULL,

    -- Foreign key constraints
    FOREIGN KEY (recording_id) REFERENCES recordings(id)
);

-- Indexes for recording_configurations
CREATE INDEX idx_recording_configurations_created ON recording_configurations(created_at);
CREATE INDEX idx_recording_configurations_active ON recording_configurations(deleted_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_recording_configurations_name ON recording_configurations(name) WHERE deleted_at IS NULL;

-- Indexes for recordings
CREATE INDEX idx_recordings_config_id ON recordings(recording_config_id);
CREATE INDEX idx_recordings_created ON recordings(created_at);
CREATE INDEX idx_recordings_active ON recordings(deleted_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_recordings_format ON recordings(format) WHERE deleted_at IS NULL;
CREATE INDEX idx_recordings_artist ON recordings(artist) WHERE deleted_at IS NULL;
CREATE INDEX idx_recordings_duration ON recordings(duration_seconds) WHERE deleted_at IS NULL;

-- Indexes for recording_output
CREATE INDEX idx_recording_output_recording_id ON recording_output(recording_id);
CREATE INDEX idx_recording_output_sequence ON recording_output(recording_id, chunk_sequence) WHERE deleted_at IS NULL;
CREATE INDEX idx_recording_output_created ON recording_output(created_at);
CREATE INDEX idx_recording_output_active ON recording_output(deleted_at) WHERE deleted_at IS NULL;