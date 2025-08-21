-- Migration: Increase buffer size to 1024 samples for even better audio quality
-- Further optimization to eliminate any remaining audio artifacts

UPDATE mixer_settings 
SET value = '1024' 
WHERE key = 'buffer_size';