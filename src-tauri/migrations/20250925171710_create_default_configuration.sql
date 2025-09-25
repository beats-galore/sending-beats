-- Create a default reusable configuration
-- This ensures there's always at least one configuration available

INSERT INTO audio_mixer_configurations (
    id,
    name,
    description,
    configuration_type,
    session_active,
    reusable_configuration_id,
    is_default,
    created_at,
    updated_at,
    deleted_at
) VALUES (
    '550e8400-e29b-41d4-a716-446655440000',
    'Default Configuration',
    'Default audio mixer configuration with standard settings',
    'reusable',
    false,
    null,
    true,
    datetime('now'),
    datetime('now'),
    null
) ON CONFLICT (id) DO NOTHING;