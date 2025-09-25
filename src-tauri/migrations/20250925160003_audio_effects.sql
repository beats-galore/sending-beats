-- Audio effects tables for default and custom effects
-- Separates basic channel effects from complex custom effects

-- Default audio effects - basic channel controls
CREATE TABLE audio_effects_default (
    id VARCHAR(36) PRIMARY KEY,
    device_id VARCHAR(36) NOT NULL,
    configuration_id VARCHAR(36) NOT NULL,
    gain REAL DEFAULT 0.0,          -- in dB
    pan REAL DEFAULT 0.0,           -- -1.0 (left) to 1.0 (right)
    muted BOOLEAN DEFAULT FALSE,
    solo BOOLEAN DEFAULT FALSE,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE NULL,

    -- Foreign key constraints
    FOREIGN KEY (device_id) REFERENCES configured_audio_devices(id),
    FOREIGN KEY (configuration_id) REFERENCES audio_mixer_configurations(id)
);

-- Custom audio effects - complex effects with JSON parameters
CREATE TABLE audio_effects_custom (
    id VARCHAR(36) PRIMARY KEY,
    device_id VARCHAR(36) NOT NULL,
    configuration_id VARCHAR(36) NOT NULL,
    type TEXT NOT NULL,             -- 'equalizer', 'compressor', 'limiter', etc.
    parameters JSONB,               -- Effect-specific parameters as JSON

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE NULL,

    -- Foreign key constraints
    FOREIGN KEY (device_id) REFERENCES configured_audio_devices(id),
    FOREIGN KEY (configuration_id) REFERENCES audio_mixer_configurations(id)
);

-- Indexes for audio_effects_default
CREATE INDEX idx_audio_effects_default_device_id ON audio_effects_default(device_id);
CREATE INDEX idx_audio_effects_default_config_id ON audio_effects_default(configuration_id);
CREATE INDEX idx_audio_effects_default_created ON audio_effects_default(created_at);
CREATE INDEX idx_audio_effects_default_active ON audio_effects_default(deleted_at) WHERE deleted_at IS NULL;

-- Indexes for audio_effects_custom
CREATE INDEX idx_audio_effects_custom_device_id ON audio_effects_custom(device_id);
CREATE INDEX idx_audio_effects_custom_config_id ON audio_effects_custom(configuration_id);
CREATE INDEX idx_audio_effects_custom_type ON audio_effects_custom(type) WHERE deleted_at IS NULL;
CREATE INDEX idx_audio_effects_custom_created ON audio_effects_custom(created_at);
CREATE INDEX idx_audio_effects_custom_active ON audio_effects_custom(deleted_at) WHERE deleted_at IS NULL;