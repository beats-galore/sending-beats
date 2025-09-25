-- Audio devices configuration table
-- Tracks configured audio input/output devices for mixer channels

CREATE TABLE configured_audio_devices (
    id VARCHAR(36) PRIMARY KEY,
    device_identifier TEXT NOT NULL,  -- Device name/identifier from system
    device_name TEXT,                 -- Human-readable device name
    sample_rate INTEGER NOT NULL,
    buffer_size INTEGER,
    channel_format TEXT NOT NULL,    -- 'stereo' or 'mono', not enforced in DB
    is_virtual BOOLEAN DEFAULT FALSE NOT NULL,
    is_input BOOLEAN DEFAULT TRUE NOT NULL,
    configuration_id VARCHAR(36) NOT NULL,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE NULL,

    -- Foreign key constraints
    FOREIGN KEY (configuration_id) REFERENCES audio_mixer_configurations(id)
);

-- Indexes for audio devices
CREATE INDEX idx_configured_audio_devices_config_id ON configured_audio_devices(configuration_id);
CREATE INDEX idx_configured_audio_devices_created ON configured_audio_devices(created_at);
CREATE INDEX idx_configured_audio_devices_active ON configured_audio_devices(deleted_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_configured_audio_devices_device_id ON configured_audio_devices(device_identifier) WHERE deleted_at IS NULL;
CREATE INDEX idx_configured_audio_devices_input_type ON configured_audio_devices(is_input, configuration_id) WHERE deleted_at IS NULL;