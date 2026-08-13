import type { RecordingFormat } from '../../../types/audio.types';

// The backend models the recording format as a tagged object; the interface
// offers it as a flat list of choices. These two functions are the only place
// that mapping lives.

export const FORMAT_CHOICES = ['MP3 192', 'MP3 320', 'FLAC lossless', 'WAV 24-bit'] as const;
export type FormatChoice = (typeof FORMAT_CHOICES)[number];

export const formatLabel = (format: RecordingFormat | undefined): FormatChoice => {
  if (format?.mp3) {
    return format.mp3.bitrate >= 320 ? 'MP3 320' : 'MP3 192';
  }
  if (format?.flac) {
    return 'FLAC lossless';
  }
  return 'WAV 24-bit';
};

export const formatFromLabel = (label: string): RecordingFormat => {
  switch (label) {
    case 'MP3 320':
      return { mp3: { bitrate: 320 } };
    case 'FLAC lossless':
      return { flac: { compression_level: 5 } };
    case 'WAV 24-bit':
      return { wav: {} };
    case 'MP3 192':
    default:
      return { mp3: { bitrate: 192 } };
  }
};
