-- Core schema for Sendin Beats database
-- Creates the fundamental tables for audio mixer configurations

-- Audio mixer configurations - parent mapping table
CREATE TABLE audio_mixer_configurations (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    configuration_type TEXT NOT NULL,         -- 'reusable' or 'session' (application enum, not DB enforced)

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP WITH TIME ZONE NULL
);

-- Indexes for mixer configurations
CREATE INDEX idx_mixer_config_created ON audio_mixer_configurations(created_at);
CREATE INDEX idx_mixer_config_active ON audio_mixer_configurations(deleted_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_mixer_config_name ON audio_mixer_configurations(name) WHERE deleted_at IS NULL;
CREATE INDEX idx_mixer_config_type ON audio_mixer_configurations(configuration_type) WHERE deleted_at IS NULL;