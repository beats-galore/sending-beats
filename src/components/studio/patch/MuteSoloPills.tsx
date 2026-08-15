import { Pill } from '../primitives/Pill';

type MuteSoloPillsProps = {
  muted: boolean;
  solo: boolean;
  onMute: () => void;
  onSolo: () => void;
};

/**
 * The two flags every source keeps, at every size.
 *
 * A node shrunk to nothing else still carries these: silencing a source is the
 * one thing you reach for without wanting to read anything first.
 */
export const MuteSoloPills = ({ muted, solo, onMute, onSolo }: MuteSoloPillsProps) => (
  <>
    <Pill
      tone={muted ? 'hot' : 'muted'}
      filled={muted}
      onClick={(event) => {
        event.stopPropagation();
        onMute();
      }}
    >
      M
    </Pill>
    <Pill
      tone={solo ? 'warn' : 'muted'}
      filled={solo}
      onClick={(event) => {
        event.stopPropagation();
        onSolo();
      }}
    >
      S
    </Pill>
  </>
);
