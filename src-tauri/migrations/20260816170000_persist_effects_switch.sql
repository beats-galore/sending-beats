-- The channel's effects switch, persisted with the settings it gates.
--
-- The chain's knobs already survive a restart on this row; the switch that
-- turns the chain on was interface-only state, so a relaunched channel came
-- back configured but switched off. Now the whole strip comes back as it was
-- left.

ALTER TABLE audio_effects_default ADD COLUMN effects_enabled BOOLEAN NOT NULL DEFAULT FALSE;
