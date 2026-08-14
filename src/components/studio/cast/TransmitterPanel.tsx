import { Panel } from '../primitives/Panel';
import { PanelHeading } from '../primitives/PanelHeading';
import { Pill } from '../primitives/Pill';
import { TransmitterControls } from './TransmitterControls';

type TransmitterPanelProps = {
  isLive: boolean;
  isBusy: boolean;
  onToggle: () => void;
};

/** The transmitter controls as the CAST view shows them, on a titled panel. */
export const TransmitterPanel = ({ isLive, isBusy, onToggle }: TransmitterPanelProps) => (
  <Panel
    p="4xl"
    gap="2xl"
    title={<PanelHeading order={2}>THE TRANSMITTER</PanelHeading>}
    action={
      <Pill tone={isLive ? 'accent' : 'neutral'} filled={isLive} size="2xs">
        {isLive ? 'CONNECTED' : 'DISCONNECTED'}
      </Pill>
    }
  >
    <TransmitterControls density="roomy" isLive={isLive} isBusy={isBusy} onToggle={onToggle} />
  </Panel>
);
