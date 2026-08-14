-- Names the user gives to mixer channels.
--
-- A channel had no representation in the database at all: the strips came from
-- create_dj_config() in Rust and were rebuilt on every launch, so a name could
-- not survive a restart.
--
-- These hang off a configuration the same way configured_audio_devices do, and
-- deliberately not off the device row — switching a channel's source deletes and
-- recreates that row, which would discard the name every time the source
-- changed. The name belongs to the channel, which outlives the device patched
-- into it.

CREATE TABLE mixer_channels (
    id VARCHAR(36) PRIMARY KEY,
    configuration_id VARCHAR(36) NOT NULL,
    channel_number INTEGER NOT NULL,
    name TEXT NOT NULL,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (configuration_id) REFERENCES audio_mixer_configurations(id)
);

CREATE INDEX idx_mixer_channels_configuration_id ON mixer_channels(configuration_id);
CREATE INDEX idx_mixer_channels_created ON mixer_channels(created_at);

-- One name per channel within a configuration
CREATE UNIQUE INDEX idx_mixer_channels_config_channel
    ON mixer_channels(configuration_id, channel_number);
