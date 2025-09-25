-- Audio device levels table for VU meter data
-- Tracks real-time audio levels for each configured device

CREATE TABLE audio_device_levels (
    id VARCHAR(36) PRIMARY KEY,
    audio_device_id VARCHAR(36) NOT NULL,
    configuration_id VARCHAR(36) NOT NULL,
    peak_left REAL NOT NULL,        -- Peak level for left channel (-∞ to 0 dB)
    peak_right REAL NOT NULL,       -- Peak level for right channel (-∞ to 0 dB)
    rms_left REAL NOT NULL,         -- RMS level for left channel (-∞ to 0 dB)
    rms_right REAL NOT NULL,        -- RMS level for right channel (-∞ to 0 dB)

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE NULL,

    -- Foreign key constraints
    FOREIGN KEY (audio_device_id) REFERENCES configured_audio_devices(id),
    FOREIGN KEY (configuration_id) REFERENCES audio_mixer_configurations(id)
);

-- Indexes for audio_device_levels
CREATE INDEX idx_audio_device_levels_device_id ON audio_device_levels(audio_device_id);
CREATE INDEX idx_audio_device_levels_config_id ON audio_device_levels(configuration_id);
CREATE INDEX idx_audio_device_levels_created ON audio_device_levels(created_at);
CREATE INDEX idx_audio_device_levels_active ON audio_device_levels(deleted_at) WHERE deleted_at IS NULL;

-- Composite index for recent levels query optimization
CREATE INDEX idx_audio_device_levels_recent ON audio_device_levels(audio_device_id, created_at DESC) WHERE deleted_at IS NULL;