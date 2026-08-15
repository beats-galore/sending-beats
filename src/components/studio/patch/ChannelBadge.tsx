import { Box } from '@mantine/core';

import { channelTargetKey } from '../../../services/patch-color-service';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { usePatchColor } from '../hooks/use-patch-color';
import { SwatchPicker } from '../primitives/SwatchPicker';

type ChannelBadgeProps = {
  /** The channel number, which is what the colour is stored against */
  channelNumber: number;
  /** Position in the source column, used to derive a colour before one is picked */
  index: number;
  /** Muted and unavailable strips grey out, so the colour is not the whole story */
  dimmed: boolean;
};

/** The numbered tag on a source, and where its colour is chosen. */
export const ChannelBadge = ({ channelNumber, index, dimmed }: ChannelBadgeProps) => {
  const swatch = usePatchColor(channelTargetKey(channelNumber), index);
  const background = dimmed ? color.dead : swatch.value;

  return (
    <SwatchPicker
      selected={swatch.key}
      assigned={swatch.assigned}
      onSelect={swatch.select}
      onReset={swatch.reset}
      label="SOURCE COLOUR"
    >
      <Box
        fz="3xs"
        title="Choose this source's colour"
        style={{
          padding: '2px 7px',
          borderRadius: 'var(--mantine-radius-xs)',
          fontWeight: 600,
          letterSpacing: layout.tracking.wide,
          whiteSpace: 'nowrap',
          border: `1px solid ${background}`,
          background,
          color: dimmed ? color.textDim : color.bg,
        }}
      >
        {String(index + 1).padStart(2, '0')}
      </Box>
    </SwatchPicker>
  );
};
