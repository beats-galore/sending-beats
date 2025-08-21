-- Migration: Update default buffer size for better audio quality
-- Increase from 256 to 512 samples to reduce crunchiness and dropouts

UPDATE mixer_settings 
SET value = '512' 
WHERE key = 'buffer_size';