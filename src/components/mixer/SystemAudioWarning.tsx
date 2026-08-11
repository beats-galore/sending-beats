import { Alert } from '@mantine/core';
import { IconAlertTriangle } from '@tabler/icons-react';
import { useCallback } from 'react';

import { useMixerStore } from '../../stores/mixer-store';

/**
 * Non-blocking notice shown when the output device switched but system audio
 * could not be diverted to the virtual driver, which makes every source audible
 * twice: once from the application itself and once through the mixer.
 */
export const SystemAudioWarning = () => {
  const warning = useMixerStore((state) => state.systemAudioWarning);
  const clearSystemAudioWarning = useMixerStore((state) => state.clearSystemAudioWarning);

  const handleDismiss = useCallback(() => {
    clearSystemAudioWarning();
  }, [clearSystemAudioWarning]);

  if (!warning) {
    return null;
  }

  return (
    <Alert
      icon={<IconAlertTriangle size={16} />}
      title="System audio not routed through Sendin Beats"
      color="yellow"
      withCloseButton
      onClose={handleDismiss}
    >
      Your output device is active, but system audio is still going to your previous output, so you
      will hear each source twice. Reselect the output device to try again.
      <br />
      {warning}
    </Alert>
  );
};
