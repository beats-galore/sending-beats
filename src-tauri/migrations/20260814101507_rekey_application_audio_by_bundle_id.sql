-- Re-key application audio sources from PID to bundle identifier.
--
-- Sources were stored as 'app-{pid}'. A PID is only meaningful for the lifetime
-- of one launch, so every saved application source failed to reconnect on the
-- next startup. They are now stored as 'app-{bundle_identifier}'.
--
-- Every 'app-%' row predates this change and is therefore PID-keyed: bundle-keyed
-- rows only start being written once this migration has run.
--
-- A row is recoverable when its saved device_name still matches a known
-- application: the name was copied from audio_applications.application_name when
-- the source was configured. Unrecoverable rows are dropped first so the re-key
-- below cannot match rows it has already rewritten.

DELETE FROM audio_effects_default
WHERE device_id IN (
    SELECT cad.id
    FROM configured_audio_devices cad
    WHERE cad.device_identifier LIKE 'app-%'
      AND NOT EXISTS (
            SELECT 1
            FROM audio_applications aa
            WHERE aa.application_name = cad.device_name
        )
);

DELETE FROM configured_audio_devices
WHERE device_identifier LIKE 'app-%'
  AND NOT EXISTS (
        SELECT 1
        FROM audio_applications aa
        WHERE aa.application_name = configured_audio_devices.device_name
    );

-- Everything still prefixed 'app-' now has a matching application, so the
-- subquery is guaranteed to resolve.
UPDATE configured_audio_devices
SET device_identifier = 'app-' || (
        SELECT aa.bundle_identifier
        FROM audio_applications aa
        WHERE aa.application_name = configured_audio_devices.device_name
        LIMIT 1
    ),
    updated_at = CURRENT_TIMESTAMP
WHERE device_identifier LIKE 'app-%';
