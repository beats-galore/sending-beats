// Hook specifically for master section data - only what MasterSection needs
import { useCallback } from 'react';

import { useMixerStore } from '../stores';

import type { ConfiguredAudioDevice } from '../types/db';
import type { Identifier } from '../types/util.types';

export const useMasterSectionData = () => {
  // Only select the master-related config data
  const mixerConfig = useMixerStore((state) => state.config);
  const setMasterGainAction = useMixerStore((state) => state.updateMasterGain);
  const setMasterOutputDeviceAction = useMixerStore((state) => state.updateMasterOutputDevice);

  const setMasterGain = useCallback(
    (gain: number) => {
      void setMasterGainAction(gain);
    },
    [setMasterGainAction]
  );

  const setMasterOutputDevice = useCallback(
    (deviceId: Identifier<ConfiguredAudioDevice>) => {
      void setMasterOutputDeviceAction(deviceId);
    },
    [setMasterOutputDeviceAction]
  );

  return {
    mixerConfig,
    setMasterGain,
    setMasterOutputDevice,
  };
};
