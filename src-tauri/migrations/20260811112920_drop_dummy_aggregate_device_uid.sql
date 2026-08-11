-- Drop the dummy aggregate device column from system_audio_state
--
-- System audio diversion is handled by the SendinBeatsAudio HAL virtual driver,
-- which supersedes the earlier silent-aggregate-device approach. The only writer
-- of this column sat on an unreachable code path, so it was never populated and
-- no orphaned aggregate devices can exist to clean up.

ALTER TABLE system_audio_state DROP COLUMN dummy_aggregate_device_uid;
