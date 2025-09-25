-- Add default field to audio_mixer_configurations
-- This allows marking a reusable configuration as the default one to load on startup

-- Add is_default boolean column (only one can be true for reusable configurations)
ALTER TABLE audio_mixer_configurations
ADD COLUMN is_default BOOLEAN DEFAULT FALSE NOT NULL;

-- Add unique constraint to ensure only one default reusable configuration
CREATE UNIQUE INDEX idx_mixer_config_single_default_reusable ON audio_mixer_configurations(is_default)
WHERE is_default = TRUE AND configuration_type = 'reusable' AND deleted_at IS NULL;

-- Add index for querying default configuration
CREATE INDEX idx_mixer_config_default ON audio_mixer_configurations(is_default, configuration_type) WHERE is_default = TRUE AND deleted_at IS NULL;