-- Where a player stops on its own.
--
-- "Pause after this one" — the end of an ad run, or the last record before a
-- guest arrives. Held as the track it follows rather than a position, so a queue
-- reordered while it is playing still pauses in the right place, and removing
-- that track removes the instruction with it.
--
-- Nullable because most players never have one. The foreign key is what clears
-- it when the track goes.

ALTER TABLE file_players
    ADD COLUMN breakpoint_track_id VARCHAR(36)
    REFERENCES file_player_tracks(id) ON DELETE SET NULL;
