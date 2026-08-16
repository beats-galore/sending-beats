-- Queues become a thing of their own, the way stations are.
--
-- They were owned by a configuration: a queue existed inside one patch and
-- nowhere else. That is the wrong shape for what they are actually used for —
-- the same run of ads belongs to the station, not to whichever patch happened
-- to be loaded when it was built. So a queue is now global and a patch merely
-- refers to one, exactly as it refers to a cast configuration.
--
-- And a queue is a list rather than something that empties. Playing a track no
-- longer takes it out; the list stays as it was built and the plays are written
-- down beside it. An ad break is worth keeping and running again tomorrow.
--
-- Dropped and rebuilt rather than migrated. The old rows were shaped around a
-- queue that consumed itself, and there is nothing in them worth carrying into
-- a model that does not.

DROP TABLE IF EXISTS file_player_tracks;
DROP TABLE IF EXISTS file_players;

-- A named queue of audio files, belonging to the studio rather than to a patch
CREATE TABLE file_players (
    id VARCHAR(36) PRIMARY KEY,
    name TEXT NOT NULL,

    -- What the queue emits, whatever its files are recorded at. Every track is
    -- brought to this, so a list of mixed formats leaves as one steady stream.
    sample_rate INTEGER NOT NULL,
    channels INTEGER NOT NULL,

    volume REAL NOT NULL DEFAULT 1.0,

    -- 'none' | 'track' | 'queue'. Application enum, not a DB one.
    repeat_mode TEXT NOT NULL DEFAULT 'none',
    shuffle BOOLEAN NOT NULL DEFAULT 0,

    -- The track this queue pauses after, when one has been asked for
    breakpoint_track_id VARCHAR(36),

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE INDEX idx_file_players_created ON file_players(created_at);
CREATE INDEX idx_file_players_name ON file_players(name);

-- What is in a queue, in the order it plays.
--
-- Durable: a track stays here once it has played. `position` is the whole of
-- the order, and there is no status — what has been played is recorded next
-- door rather than by taking rows out of this list.
CREATE TABLE file_player_tracks (
    id VARCHAR(36) PRIMARY KEY,
    file_player_id VARCHAR(36) NOT NULL,

    file_path TEXT NOT NULL,

    -- Read from the file when it is added. Absent for a file that carries no
    -- tags, which is not a reason to refuse to queue it.
    title TEXT,
    artist TEXT,
    album TEXT,
    duration_ms INTEGER,
    file_size INTEGER NOT NULL DEFAULT 0,

    position INTEGER NOT NULL DEFAULT 0,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (file_player_id) REFERENCES file_players(id) ON DELETE CASCADE
);

CREATE INDEX idx_file_player_tracks_order
    ON file_player_tracks(file_player_id, position);

-- Every time a queue played something.
--
-- A log, not a second copy of the list: one row per play, so a track played
-- three times reads as three plays. What the track was is written down here as
-- well as pointed at, because the log has to still make sense after the track
-- is taken out of the queue it came from.
CREATE TABLE file_player_plays (
    id VARCHAR(36) PRIMARY KEY,
    file_player_id VARCHAR(36) NOT NULL,

    -- The track it was, while that track is still in the queue
    track_id VARCHAR(36),

    file_path TEXT NOT NULL,
    title TEXT,
    artist TEXT,
    duration_ms INTEGER,

    played_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (file_player_id) REFERENCES file_players(id) ON DELETE CASCADE
);

CREATE INDEX idx_file_player_plays_when
    ON file_player_plays(file_player_id, played_at);

-- Which queues a patch has on its canvas.
--
-- The queue is global; this is the patch's side of it, so one can be put on a
-- canvas and taken off it like any other source.
CREATE TABLE configuration_file_players (
    id VARCHAR(36) PRIMARY KEY,
    configuration_id VARCHAR(36) NOT NULL,
    file_player_id VARCHAR(36) NOT NULL,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (configuration_id) REFERENCES audio_mixer_configurations(id),
    FOREIGN KEY (file_player_id) REFERENCES file_players(id) ON DELETE CASCADE
);

CREATE INDEX idx_configuration_file_players_configuration
    ON configuration_file_players(configuration_id);

-- A queue is on a patch once, not twice
CREATE UNIQUE INDEX idx_configuration_file_players_unique
    ON configuration_file_players(configuration_id, file_player_id);
