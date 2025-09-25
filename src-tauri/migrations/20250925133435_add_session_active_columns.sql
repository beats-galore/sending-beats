-- Add session_active and reusable_configuration_id columns to audio_mixer_configurations
-- These columns support the session/reusable configuration workflow

-- Add session_active boolean column (only one can be true)
ALTER TABLE audio_mixer_configurations
ADD COLUMN session_active BOOLEAN DEFAULT FALSE NOT NULL;

-- Add nullable reusable_configuration_id column for self-referential relationship
ALTER TABLE audio_mixer_configurations
ADD COLUMN reusable_configuration_id VARCHAR(36) NULL;

-- Add indexes for the new columns
CREATE INDEX idx_mixer_config_session_active ON audio_mixer_configurations(session_active) WHERE session_active = TRUE AND deleted_at IS NULL;
CREATE INDEX idx_mixer_config_reusable_ref ON audio_mixer_configurations(reusable_configuration_id) WHERE reusable_configuration_id IS NOT NULL AND deleted_at IS NULL;

-- Add unique constraint to ensure only one active session
CREATE UNIQUE INDEX idx_mixer_config_single_active_session ON audio_mixer_configurations(session_active)
WHERE session_active = TRUE AND deleted_at IS NULL;