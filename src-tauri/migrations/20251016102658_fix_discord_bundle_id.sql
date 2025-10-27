-- Fix incorrect Discord bundle identifier

UPDATE audio_applications
SET bundle_identifier = 'com.hnc.Discord'
WHERE bundle_identifier = 'com.discord.Discord';


INSERT INTO audio_applications (id, bundle_identifier, application_name, operating_system) VALUES
('fb831e52-5561-41d5-ab3f-97d21ed5b44f', 'com.serato.seratodj', 'Serato DJ', 'macos');
