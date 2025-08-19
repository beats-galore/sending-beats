// Hook specifically for master section data - only what MasterSection needs
import { useCallback } from 'react';
import { useMixerStore } from '../stores';

export const useMasterSectionData = () => {
  // Only select the master-related config data
  const mixerConfig = useMixerStore((state) => state.config);
  const setMasterGainAction = useMixerStore((state) => state.updateMasterGain);
  
  const setMasterGain = useCallback((gain: number) => {
    void setMasterGainAction(gain);
  }, [setMasterGainAction]);
  
  return {
    mixerConfig,
    setMasterGain,
  };
};