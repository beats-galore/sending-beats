-- Somewhere to broadcast to, kept so it can be chosen rather than retyped.
--
-- Deliberately not hung off a configuration, unlike buses and patchbay colours.
-- A station is a place in the world rather than part of a session: the same
-- server is streamed to from whichever patch happens to be loaded, and making it
-- belong to one would mean entering the same details again for every patch.
--
-- The row's own key is what the mixer names the stream by. Icecast previously
-- registered its output under a fresh uuid every time it went live, so a saved
-- routing could never point at the broadcast twice; taking the identifier from
-- here instead gives it one that survives going off air and back on.

CREATE TABLE cast_configurations (
    id VARCHAR(36) PRIMARY KEY,

    -- What the DJ calls it, not what listeners see. `stream_name` is that.
    name TEXT NOT NULL,

    -- Where to connect
    server_host TEXT NOT NULL,
    server_port INTEGER NOT NULL,
    mount_point TEXT NOT NULL,
    username TEXT NOT NULL DEFAULT 'source',

    -- The password is not here on purpose. It lives in the keychain under this
    -- row's id, so the database stays something that can be copied or backed up
    -- without carrying a credential with it.

    -- What listeners are shown by the server
    stream_name TEXT NOT NULL DEFAULT '',
    stream_description TEXT NOT NULL DEFAULT '',
    stream_genre TEXT NOT NULL DEFAULT '',
    stream_url TEXT NOT NULL DEFAULT '',
    is_public BOOLEAN NOT NULL DEFAULT 0,

    -- Encoding. Format is an application enum, so text rather than a constraint.
    audio_format TEXT NOT NULL DEFAULT 'mp3',
    bitrate_kbps INTEGER NOT NULL DEFAULT 192,
    variable_bitrate BOOLEAN NOT NULL DEFAULT 0,
    vbr_quality INTEGER NOT NULL DEFAULT 4,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE INDEX idx_cast_configurations_created ON cast_configurations(created_at);

-- Listing them for the picker is by name, so it is worth ordering by
CREATE INDEX idx_cast_configurations_name ON cast_configurations(name);
