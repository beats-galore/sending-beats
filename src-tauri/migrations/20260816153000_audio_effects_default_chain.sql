-- The custom effects chain settings, on the row that already carries a
-- channel's gain, pan, mute and solo.
--
-- One row per configured device per configuration, loaded when the device is
-- attached — so the chain comes back the way it was left, the same way the
-- fader does. Defaults match the DSP constructors: EQ flat, dynamics bypassed.

ALTER TABLE audio_effects_default ADD COLUMN eq_low_gain REAL NOT NULL DEFAULT 0.0;    -- in dB
ALTER TABLE audio_effects_default ADD COLUMN eq_mid_gain REAL NOT NULL DEFAULT 0.0;    -- in dB
ALTER TABLE audio_effects_default ADD COLUMN eq_high_gain REAL NOT NULL DEFAULT 0.0;   -- in dB

ALTER TABLE audio_effects_default ADD COLUMN comp_threshold REAL NOT NULL DEFAULT -12.0; -- in dB
ALTER TABLE audio_effects_default ADD COLUMN comp_ratio REAL NOT NULL DEFAULT 4.0;
ALTER TABLE audio_effects_default ADD COLUMN comp_attack REAL NOT NULL DEFAULT 10.0;     -- in ms
ALTER TABLE audio_effects_default ADD COLUMN comp_release REAL NOT NULL DEFAULT 200.0;   -- in ms
ALTER TABLE audio_effects_default ADD COLUMN comp_enabled BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE audio_effects_default ADD COLUMN limiter_threshold REAL NOT NULL DEFAULT -0.1; -- in dB
ALTER TABLE audio_effects_default ADD COLUMN limiter_enabled BOOLEAN NOT NULL DEFAULT FALSE;
