// TypeScript types for application audio capture functionality

export type ProcessInfo = {
  pid: number;
  name: string;
  bundle_id?: string;
  icon_path?: string;
  is_audio_capable: boolean;
  is_playing_audio: boolean;
};

export type AvailableApplicationInfo = {
  pid: number;
  bundleIdentifier: string;
  applicationName: string;
  isInDatabase: boolean;
};
