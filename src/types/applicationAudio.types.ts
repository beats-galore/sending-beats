// TypeScript types for application audio capture functionality

export interface ProcessInfo {
  pid: number;
  name: string;
  bundle_id?: string;
  icon_path?: string;
  is_audio_capable: boolean;
  is_playing_audio: boolean;
}

export interface ApplicationAudioError {
  type: 'PermissionDenied' | 'ApplicationNotFound' | 'CoreAudioError' | 'UnsupportedSystem' | 'TooManyCaptures' | 'TapNotInitialized' | 'SystemError';
  message: string;
  details?: {
    pid?: number;
    status?: number;
    max?: number;
  };
}

export interface ApplicationAudioSource {
  type: 'application';
  id: string; // Format: "app-{pid}"
  pid: number;
  name: string;
  displayName: string; // Format: "App: {name}"
  bundleId?: string;
  iconPath?: string;
  isKnown: boolean; // True for recognized apps like Spotify, iTunes
  isPlaying: boolean;
  isCapturing: boolean;
}

export interface HardwareAudioSource {
  type: 'hardware';
  id: string;
  name: string;
  displayName: string;
  isDefault: boolean;
  hostApi: string;
  supportedChannels: number[];
}

export type AudioSource = ApplicationAudioSource | HardwareAudioSource;

export interface AudioSourceGroup {
  label: string;
  sources: AudioSource[];
}

// Utility functions for working with audio sources
export const createApplicationSource = (processInfo: ProcessInfo, isCapturing = false): ApplicationAudioSource => ({
  type: 'application',
  id: `app-${processInfo.pid}`,
  pid: processInfo.pid,
  name: processInfo.name,
  displayName: `App: ${processInfo.name}`,
  bundleId: processInfo.bundle_id,
  iconPath: processInfo.icon_path,
  isKnown: !!processInfo.bundle_id,
  isPlaying: processInfo.is_playing_audio,
  isCapturing,
});

export const createHardwareSource = (device: any): HardwareAudioSource => ({
  type: 'hardware',
  id: device.id,
  name: device.name,
  displayName: device.name + (device.is_default ? ' (Default)' : ''),
  isDefault: device.is_default,
  hostApi: device.host_api,
  supportedChannels: device.supported_channels,
});

export const groupAudioSources = (sources: AudioSource[]): AudioSourceGroup[] => {
  const hardwareSources = sources.filter((s): s is HardwareAudioSource => s.type === 'hardware');
  const knownAppSources = sources.filter((s): s is ApplicationAudioSource => s.type === 'application' && s.isKnown);
  const otherAppSources = sources.filter((s): s is ApplicationAudioSource => s.type === 'application' && !s.isKnown);
  
  const groups: AudioSourceGroup[] = [];
  
  if (hardwareSources.length > 0) {
    groups.push({
      label: 'Hardware Devices',
      sources: hardwareSources,
    });
  }
  
  if (knownAppSources.length > 0) {
    groups.push({
      label: 'Audio Applications',
      sources: knownAppSources,
    });
  }
  
  if (otherAppSources.length > 0) {
    groups.push({
      label: 'Other Applications',
      sources: otherAppSources,
    });
  }
  
  return groups;
};