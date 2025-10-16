-- Audio applications table for managing known audio-capable applications

CREATE TABLE audio_applications (
    id VARCHAR(36) PRIMARY KEY,
    bundle_identifier TEXT NOT NULL UNIQUE,
    application_name TEXT NOT NULL,
    operating_system TEXT NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    notes TEXT,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Indexes for performance
CREATE INDEX idx_audio_applications_bundle_id ON audio_applications(bundle_identifier);
CREATE INDEX idx_audio_applications_os ON audio_applications(operating_system);
CREATE INDEX idx_audio_applications_enabled ON audio_applications(is_enabled);

-- Seed with known audio applications for macOS
INSERT INTO audio_applications (id, bundle_identifier, application_name, operating_system) VALUES
    -- Music & Streaming
    ('a1b2c3d4-0001-4000-8000-000000000001', 'com.spotify.client', 'Spotify', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000002', 'com.apple.Music', 'Apple Music', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000003', 'com.apple.podcasts', 'Apple Podcasts', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000004', 'com.apple.TV', 'Apple TV', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000005', 'com.tidal.desktop', 'Tidal', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000006', 'com.soundcloud.desktop', 'SoundCloud', 'macos'),

    -- Browsers (web audio)
    ('a1b2c3d4-0001-4000-8000-000000000010', 'com.google.Chrome', 'Google Chrome', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000011', 'org.mozilla.firefox', 'Firefox', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000012', 'com.brave.Browser', 'Brave Browser', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000013', 'com.microsoft.edgemac', 'Microsoft Edge', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000014', 'com.apple.Safari', 'Safari', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000015', 'com.operasoftware.Opera', 'Opera', 'macos'),

    -- Communication
    ('a1b2c3d4-0001-4000-8000-000000000020', 'com.discord.Discord', 'Discord', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000021', 'us.zoom.xos', 'Zoom', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000022', 'com.microsoft.teams2', 'Microsoft Teams', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000023', 'com.apple.FaceTime', 'FaceTime', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000024', 'com.skype.skype', 'Skype', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000025', 'com.tinyspeck.slackmacgap', 'Slack', 'macos'),

    -- DAWs & Audio Production
    ('a1b2c3d4-0001-4000-8000-000000000030', 'com.apple.logic10', 'Logic Pro', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000031', 'com.apple.garageband10', 'GarageBand', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000032', 'com.ableton.live', 'Ableton Live', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000033', 'com.avid.ProTools', 'Pro Tools', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000034', 'com.steinberg.cubase', 'Cubase', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000035', 'com.presonus.studioone', 'Studio One', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000036', 'com.cockos.reaper', 'REAPER', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000037', 'com.bitwig.BitwigStudio', 'Bitwig Studio', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000038', 'com.image-line.FLStudio', 'FL Studio', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000039', 'com.cycling74.Max', 'Max', 'macos'),
    ('a1b2c3d4-0001-4000-8000-00000000003a', 'com.native-instruments.Maschine2', 'Maschine', 'macos'),
    ('a1b2c3d4-0001-4000-8000-00000000003b', 'com.audacityteam.audacity', 'Audacity', 'macos'),

    -- Audio Utilities
    ('a1b2c3d4-0001-4000-8000-000000000040', 'com.rogueamoeba.AudioHijack', 'Audio Hijack', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000041', 'com.rogueamoeba.Loopback', 'Loopback', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000042', 'com.rogueamoeba.farrago', 'Farrago', 'macos'),

    -- Video Players
    ('a1b2c3d4-0001-4000-8000-000000000050', 'org.videolan.vlc', 'VLC', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000051', 'io.mpv', 'mpv', 'macos'),
    ('a1b2c3d4-0001-4000-8000-000000000052', 'com.colliderli.iina', 'IINA', 'macos'),
    -- Screen Continuity
    ('a1b2c3d4-0001-4000-8000-000000000070', 'com.apple.ScreenContinuity', 'Screen Continuity', 'macos');
