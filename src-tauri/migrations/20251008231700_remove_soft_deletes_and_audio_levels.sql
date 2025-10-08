-- Remove soft delete pattern and drop audio_device_levels table

-- Drop audio_device_levels table
DROP TABLE IF EXISTS audio_device_levels;

-- Drop ALL indexes that have WHERE deleted_at clauses
-- audio_mixer_configurations
DROP INDEX IF EXISTS idx_mixer_config_active;
DROP INDEX IF EXISTS idx_mixer_config_name;
DROP INDEX IF EXISTS idx_mixer_config_type;
DROP INDEX IF EXISTS idx_mixer_config_session_active;
DROP INDEX IF EXISTS idx_mixer_config_reusable_ref;
DROP INDEX IF EXISTS idx_mixer_config_default;
DROP INDEX IF EXISTS idx_mixer_config_single_active_session;
DROP INDEX IF EXISTS idx_mixer_config_single_default_reusable;

-- configured_audio_devices
DROP INDEX IF EXISTS idx_configured_audio_devices_active;
DROP INDEX IF EXISTS idx_configured_audio_devices_device_id;
DROP INDEX IF EXISTS idx_configured_audio_devices_input_type;

-- audio_effects_default
DROP INDEX IF EXISTS idx_audio_effects_default_active;

-- audio_effects_custom
DROP INDEX IF EXISTS idx_audio_effects_custom_type;
DROP INDEX IF EXISTS idx_audio_effects_custom_active;

-- recordings (if they exist)
DROP INDEX IF EXISTS idx_recording_configurations_active;
DROP INDEX IF EXISTS idx_recording_configurations_name;
DROP INDEX IF EXISTS idx_recordings_active;
DROP INDEX IF EXISTS idx_recordings_format;
DROP INDEX IF EXISTS idx_recordings_artist;
DROP INDEX IF EXISTS idx_recordings_duration;
DROP INDEX IF EXISTS idx_recording_output_sequence;
DROP INDEX IF EXISTS idx_recording_output_active;

-- broadcasts (if they exist)
DROP INDEX IF EXISTS idx_broadcast_configurations_active;
DROP INDEX IF EXISTS idx_broadcast_configurations_name;
DROP INDEX IF EXISTS idx_broadcast_configurations_server;
DROP INDEX IF EXISTS idx_broadcasts_active;
DROP INDEX IF EXISTS idx_broadcasts_status;
DROP INDEX IF EXISTS idx_broadcasts_duration;
DROP INDEX IF EXISTS idx_broadcasts_active_sessions;
DROP INDEX IF EXISTS idx_broadcast_output_sequence;
DROP INDEX IF EXISTS idx_broadcast_output_active;

-- Drop deleted_at columns
ALTER TABLE audio_mixer_configurations DROP COLUMN deleted_at;
ALTER TABLE configured_audio_devices DROP COLUMN deleted_at;
ALTER TABLE audio_effects_default DROP COLUMN deleted_at;
ALTER TABLE audio_effects_custom DROP COLUMN deleted_at;

-- Recreate important indexes without WHERE deleted_at clauses
-- audio_mixer_configurations
CREATE INDEX idx_mixer_config_name ON audio_mixer_configurations(name);
CREATE INDEX idx_mixer_config_type ON audio_mixer_configurations(configuration_type);
CREATE INDEX idx_mixer_config_session_active ON audio_mixer_configurations(session_active) WHERE session_active = TRUE;
CREATE INDEX idx_mixer_config_reusable_ref ON audio_mixer_configurations(reusable_configuration_id) WHERE reusable_configuration_id IS NOT NULL;
CREATE INDEX idx_mixer_config_default ON audio_mixer_configurations(is_default, configuration_type) WHERE is_default = TRUE;
CREATE UNIQUE INDEX idx_mixer_config_single_active_session ON audio_mixer_configurations(session_active) WHERE session_active = TRUE;
CREATE UNIQUE INDEX idx_mixer_config_single_default_reusable ON audio_mixer_configurations(is_default) WHERE is_default = TRUE AND configuration_type = 'reusable';

-- configured_audio_devices
CREATE INDEX idx_configured_audio_devices_device_id ON configured_audio_devices(device_identifier);
CREATE INDEX idx_configured_audio_devices_input_type ON configured_audio_devices(is_input, configuration_id);

-- audio_effects_custom
CREATE INDEX idx_audio_effects_custom_type ON audio_effects_custom(type);

-- recordings (if they exist)
CREATE INDEX IF NOT EXISTS idx_recording_configurations_name ON recording_configurations(name);
CREATE INDEX IF NOT EXISTS idx_recordings_format ON recordings(format);
CREATE INDEX IF NOT EXISTS idx_recordings_artist ON recordings(artist);
CREATE INDEX IF NOT EXISTS idx_recordings_duration ON recordings(duration_seconds);
CREATE INDEX IF NOT EXISTS idx_recording_output_sequence ON recording_output(recording_id, chunk_sequence);

-- broadcasts (if they exist)
CREATE INDEX IF NOT EXISTS idx_broadcast_configurations_name ON broadcast_configurations(name);
CREATE INDEX IF NOT EXISTS idx_broadcast_configurations_server ON broadcast_configurations(server_url);
CREATE INDEX IF NOT EXISTS idx_broadcasts_status ON broadcasts(final_status);
CREATE INDEX IF NOT EXISTS idx_broadcasts_duration ON broadcasts(duration_seconds);
CREATE INDEX IF NOT EXISTS idx_broadcasts_active_sessions ON broadcasts(start_time, end_time) WHERE end_time IS NULL;
CREATE INDEX IF NOT EXISTS idx_broadcast_output_sequence ON broadcast_output(broadcast_id, chunk_sequence);
