-- File players: a queue of audio files that plays into the mixer as a source.
--
-- The motivating case is ads. A player is patched into a channel like any other
-- input, so the bus routing decides where it lands — reaching the broadcast
-- without reaching the tape is the thing this exists for.
--
-- Stored per configuration, the way buses and channel colours are: which files
-- are queued is part of what a session is, not a property of the machine.

CREATE TABLE file_players (
    id VARCHAR(36) PRIMARY KEY,
    configuration_id VARCHAR(36) NOT NULL,

    -- What the mixing layer routes by, and what a channel is patched to.
    --
    -- Derived from `id` rather than minted separately, so it survives a restart:
    -- the identifier was previously a counter that started again from one every
    -- launch, which no saved patch could point at twice.
    device_identifier TEXT NOT NULL,

    name TEXT NOT NULL,

    -- What the player emits, whatever its files are recorded at. Every track is
    -- brought to this, so a queue of mixed formats leaves as one steady stream.
    sample_rate INTEGER NOT NULL,
    channels INTEGER NOT NULL,

    volume REAL NOT NULL DEFAULT 1.0,

    -- 'none' | 'track' | 'queue'. Application enum, not a DB one.
    repeat_mode TEXT NOT NULL DEFAULT 'none',
    shuffle BOOLEAN NOT NULL DEFAULT 0,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (configuration_id) REFERENCES audio_mixer_configurations(id)
);

CREATE INDEX idx_file_players_configuration_id ON file_players(configuration_id);
CREATE INDEX idx_file_players_created ON file_players(created_at);

-- One player per identifier within a configuration, since that is what a
-- channel is patched to
CREATE UNIQUE INDEX idx_file_players_config_device
    ON file_players(configuration_id, device_identifier);

-- What a player has played and what it is going to play.
--
-- One table rather than two: history and queue differ by whether a file has been
-- through the decoder yet, and splitting them would mean moving a row every time
-- a track finishes and reconstructing the order across both to answer "what did
-- this player do tonight".
CREATE TABLE file_player_tracks (
    id VARCHAR(36) PRIMARY KEY,
    file_player_id VARCHAR(36) NOT NULL,

    file_path TEXT NOT NULL,

    -- Read from the file when it is added. Absent for a file that has moved or
    -- that carries no tags, which is not a reason to refuse to queue it.
    title TEXT,
    artist TEXT,
    album TEXT,
    duration_ms INTEGER,
    file_size INTEGER NOT NULL DEFAULT 0,

    -- 'pending' | 'played'. Application enum, not a DB one.
    status TEXT NOT NULL DEFAULT 'pending',

    -- Order within the player. Kept as the queue order for pending tracks and
    -- as the order they were played for the rest, so one column reads both ways.
    position INTEGER NOT NULL DEFAULT 0,

    -- When it last finished, so history can be read by time rather than only by
    -- order. Null while a track has never played.
    played_at TIMESTAMP WITH TIME ZONE,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (file_player_id) REFERENCES file_players(id) ON DELETE CASCADE
);

CREATE INDEX idx_file_player_tracks_player ON file_player_tracks(file_player_id);
CREATE INDEX idx_file_player_tracks_created ON file_player_tracks(created_at);

-- Reading a player's queue, and its history, are both this lookup
CREATE INDEX idx_file_player_tracks_status
    ON file_player_tracks(file_player_id, status, position);
