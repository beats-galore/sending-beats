import { Switch } from '@mantine/core';

import { useStudioStore } from '../../../stores/studio-store';
import { Panel } from '../primitives/Panel';

/** What happens when the application starts. */
export const LaunchPanel = () => {
  const launch = useStudioStore((state) => state.launch);
  const toggleLaunch = useStudioStore((state) => state.toggleLaunch);

  return (
    <Panel title="ON LAUNCH" p="3xl">
      <Switch
        checked={launch.autoStartEngine}
        onChange={() => toggleLaunch('autoStartEngine')}
        label="Start the engine automatically"
      />
      <Switch
        checked={launch.restoreLastPatch}
        onChange={() => toggleLaunch('restoreLastPatch')}
        label="Restore the last patch"
      />
    </Panel>
  );
};
