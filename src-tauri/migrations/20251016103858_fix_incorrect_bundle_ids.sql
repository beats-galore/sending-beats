-- fix incorrect bundle ids

UPDATE audio_applications
SET bundle_identifier = 'com.rogueamoeba.AudioHijack'
WHERE bundle_identifier = 'com.rogueamoeba.audiohijack3';