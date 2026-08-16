import { useMixerStore } from '../../../stores/mixer-store';
import { ActionButton } from '../primitives/ActionButton';

/** Writes the running session back over the saved patch it was loaded from. */
export const PatchSaveButton = () => {
  const saveSessionToReusable = useMixerStore((state) => state.saveSessionToReusable);
  const activeSession = useMixerStore((state) => state.activeSession);
  const linked = Boolean(activeSession?.configuration.reusableConfigurationId);

  return (
    <ActionButton
      tone="accent"
      disabled={!linked}
      padding="6px 11px"
      onClick={() => void saveSessionToReusable()}
    >
      SAVE
    </ActionButton>
  );
};
