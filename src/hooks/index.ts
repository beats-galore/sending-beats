// Barrel exports for all custom hooks
export { useAudioDevices } from './use-audio-devices';
export { useMixerState } from './use-mixer-state';

// New focused hooks for performance optimization
export { useChannelsData } from './use-channels-data';
export { useMasterSectionData } from './use-master-section-data';
export { useMixerInitialization } from './use-mixer-initialization';
export { useMixerRunningState } from './use-mixer-running-state';
export { useChannelLevels } from './use-channel-levels';
export { useMasterLevels } from './use-master-levels';
export { useAudioMetrics } from './use-audio-metrics';
export { useAudioDevicesStatus } from './use-audio-devices-status';

// Streaming hooks
export { useStreamingStatus } from './use-streaming-status';
export { useStreamingControls } from './use-streaming-controls';

// Recording hooks
export { useRecording } from './use-recording';

// Application audio hooks
export { useApplicationAudio } from './use-application-audio';
export { useAudioPermissions } from './use-audio-permissions';
