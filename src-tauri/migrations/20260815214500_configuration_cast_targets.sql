-- Which stations a patch broadcasts to.
--
-- Stations themselves are global — a place in the world, streamed to from
-- whichever patch is loaded. This is the other half of that: which of them are
-- on this patch's canvas, so a cast can be added and removed like any other
-- destination rather than always being there.
--
-- Its own table rather than a row in configured_audio_devices. A broadcast is
-- not a device: there is no hardware to enumerate, nothing to fall back to when
-- it is absent, and the columns that table needs to describe one would all be
-- empty. Keeping it apart also means the destination list stays a list of
-- devices, which is what everything reading it expects.

CREATE TABLE configuration_cast_targets (
    id VARCHAR(36) PRIMARY KEY,
    configuration_id VARCHAR(36) NOT NULL,
    cast_configuration_id VARCHAR(36) NOT NULL,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (configuration_id) REFERENCES audio_mixer_configurations(id),
    FOREIGN KEY (cast_configuration_id) REFERENCES cast_configurations(id) ON DELETE CASCADE
);

CREATE INDEX idx_configuration_cast_targets_configuration
    ON configuration_cast_targets(configuration_id);
CREATE INDEX idx_configuration_cast_targets_created
    ON configuration_cast_targets(created_at);

-- A station is on a patch once, not twice
CREATE UNIQUE INDEX idx_configuration_cast_targets_unique
    ON configuration_cast_targets(configuration_id, cast_configuration_id);
