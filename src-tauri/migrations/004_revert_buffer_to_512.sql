-- Migration: Revert buffer size back to 512 samples (1024 didn't help further)

UPDATE mixer_settings 
SET value = '512' 
WHERE key = 'buffer_size';