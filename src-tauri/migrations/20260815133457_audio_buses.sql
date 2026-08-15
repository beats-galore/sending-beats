-- Buses: which inputs feed which mix, and which outputs take it.
--
-- Routing lived only in the mixing layer, so any bus the user made vanished on
-- restart. These hang off a configuration the same way mixer_channels and
-- configured_audio_devices do, since routing is part of what a session is.
--
-- Membership is its own table rather than a list on the bus row because a
-- device belongs to a bus rather than the other way round, and because an input
-- can feed several buses at once. Members are keyed by device_identifier, the
-- same string the mixer routes by, not by configured_audio_devices.id — that
-- row is deleted and recreated whenever a channel's source is switched, which
-- would discard the routing every time the source changed.

CREATE TABLE audio_buses (
    id VARCHAR(36) PRIMARY KEY,
    configuration_id VARCHAR(36) NOT NULL,

    -- The identifier the mixing layer routes by, unique within a configuration
    bus_id TEXT NOT NULL,
    name TEXT NOT NULL,
    gain REAL NOT NULL DEFAULT 1.0,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (configuration_id) REFERENCES audio_mixer_configurations(id)
);

CREATE INDEX idx_audio_buses_configuration_id ON audio_buses(configuration_id);
CREATE INDEX idx_audio_buses_created ON audio_buses(created_at);

-- One bus per identifier within a configuration
CREATE UNIQUE INDEX idx_audio_buses_config_bus ON audio_buses(configuration_id, bus_id);

CREATE TABLE audio_bus_members (
    id VARCHAR(36) PRIMARY KEY,
    bus_row_id VARCHAR(36) NOT NULL,

    -- Matches configured_audio_devices.device_identifier, which is what the
    -- mixing layer keys its inputs and outputs by
    device_identifier TEXT NOT NULL,

    -- 'input' or 'output'. Application enum, not a DB one.
    direction TEXT NOT NULL,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (bus_row_id) REFERENCES audio_buses(id) ON DELETE CASCADE
);

CREATE INDEX idx_audio_bus_members_bus_row_id ON audio_bus_members(bus_row_id);
CREATE INDEX idx_audio_bus_members_device ON audio_bus_members(device_identifier);

-- A device is on a bus once per direction, not twice
CREATE UNIQUE INDEX idx_audio_bus_members_unique
    ON audio_bus_members(bus_row_id, device_identifier, direction);
