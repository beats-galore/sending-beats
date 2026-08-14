-- The device carrying system sounds is tracked separately from the default
-- output device. macOS lets them differ, and diversion changes both, so each
-- needs its own value to be put back.
ALTER TABLE system_audio_state ADD COLUMN previous_system_output_device_uid TEXT NULL;
