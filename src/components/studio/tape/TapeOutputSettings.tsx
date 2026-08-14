import type { useTapeTransport } from '../hooks/use-tape-transport';
import { Panel } from '../primitives/Panel';
import { TapeOutputControls } from './TapeOutputControls';

type TapeOutputSettingsProps = {
  tape: ReturnType<typeof useTapeTransport>;
};

/** The output controls as the TAPE view shows them, on a titled panel. */
export const TapeOutputSettings = ({ tape }: TapeOutputSettingsProps) => (
  <Panel title="OUTPUT" p="3xl" gap="xl">
    <TapeOutputControls density="roomy" tape={tape} />
  </Panel>
);
