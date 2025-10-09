-- System Audio State Table
-- Tracks dummy aggregate device and system audio routing state

CREATE TABLE system_audio_state (
    id VARCHAR(36) PRIMARY KEY,
    dummy_aggregate_device_uid TEXT NULL,  -- UID of created aggregate device
    previous_default_device_uid TEXT NULL, -- UID of device to restore
    is_diverted BOOLEAN DEFAULT FALSE,     -- Currently using dummy device
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Index for querying active state
CREATE INDEX idx_system_audio_state_diverted ON system_audio_state(is_diverted);
