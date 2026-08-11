import { Group, Text } from '@mantine/core';
import { invoke } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';

import { color } from '../../../theme/tokens';
import { Panel } from '../primitives/Panel';
import { StatusDot } from '../primitives/StatusDot';

/**
 * The virtual output driver, which other applications can select as an input to
 * hear the mix. `get_system_audio_status` reports whether system audio is
 * currently diverted through it.
 */
export const VirtualDevicePanel = () => {
  const [diverted, setDiverted] = useState<boolean | null>(null);

  useEffect(() => {
    void invoke<boolean>('get_system_audio_status')
      .then(setDiverted)
      .catch(() => setDiverted(null));
  }, []);

  const label = diverted === null ? 'UNKNOWN' : diverted ? 'DIVERTED' : 'IDLE';

  return (
    <Panel title="VIRTUAL DEVICE" p="3xl">
      <Group gap="md" wrap="nowrap">
        <StatusDot size={8} tone={diverted ? 'accent' : 'inert'} />
        <Text size="md" style={{ flex: 1 }}>
          Sendin Beats Audio
        </Text>
        <Text size="2xs" c={diverted ? color.acc : color.textFaint}>
          {label}
        </Text>
      </Group>

      <Text size="xs" c={color.textDim} lh="lg">
        Other apps can select this as an input to hear your mix. System audio is routed through it
        while an output device is engaged.
      </Text>
    </Panel>
  );
};
