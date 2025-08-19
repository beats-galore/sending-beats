// Hook for audio devices status only - loading and error states
import { useAudioDeviceStore } from '../stores';

export const useAudioDevicesStatus = () => {
  // Only select loading and error states, nothing else
  const isLoading = useAudioDeviceStore((state) => state.isLoading);
  const error = useAudioDeviceStore((state) => state.error);
  
  return {
    isLoading,
    error,
  };
};