-- Colours the user gives to the things on the patchbay.
--
-- Every input and destination carries a colour, and the tiles that say where a
-- signal goes are painted with the colour of the thing they refer to. Deriving
-- it from position, as the design prototype does, means the colours shuffle as
-- soon as a channel is removed, so an assignment has to be stored.
--
-- One table for both sides rather than a column on each, because the things
-- being coloured have no common row to hang it off: a channel is a strip, an
-- output is a device, and the stream and the tape are neither. They do all have
-- a key on the patchbay, so that is what this is keyed by:
--
--   ch:<channel number>       an input strip
--   out:<device identifier>   a hardware destination
--   stream / rec              the broadcast and the tape
--
-- Deliberately not keyed by configured_audio_devices.id for inputs: that row is
-- deleted and recreated whenever a channel's source is switched, which would
-- reset the colour every time the source changed. The colour belongs to the
-- strip, which outlives the device patched into it — the same reasoning that
-- put channel names in mixer_channels.

CREATE TABLE patch_colors (
    id VARCHAR(36) PRIMARY KEY,
    configuration_id VARCHAR(36) NOT NULL,

    -- What is being coloured, in the patchbay's own key vocabulary
    target_key TEXT NOT NULL,

    -- Whatever the interface understands as a colour, stored as written
    color TEXT NOT NULL,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (configuration_id) REFERENCES audio_mixer_configurations(id)
);

CREATE INDEX idx_patch_colors_configuration_id ON patch_colors(configuration_id);
CREATE INDEX idx_patch_colors_created ON patch_colors(created_at);

-- One colour per thing within a configuration
CREATE UNIQUE INDEX idx_patch_colors_config_target
    ON patch_colors(configuration_id, target_key);
