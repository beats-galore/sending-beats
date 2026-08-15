import { Box } from '@mantine/core';

import type { PatchTargetKey } from '../../../services/patch-color-service';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';
import { usePatchColor } from '../hooks/use-patch-color';
import { SwatchPicker } from '../primitives/SwatchPicker';

type PatchBadgeProps = {
  /** What the colour is stored against: `ch:<n>`, `out:<id>`, `stream` or `rec` */
  targetKey: PatchTargetKey;
  /**
   * Where this sits in its column. Shown as the number, and used to derive a
   * colour until one is picked.
   */
  position: number;
  /** Muted, offline and idle things grey out, so colour is not the whole story */
  dimmed: boolean;
  /** Heading on the picker, naming what is being coloured */
  label: string;
};

/** The numbered tag on a source or destination, and where its colour is chosen. */
export const PatchBadge = ({ targetKey, position, dimmed, label }: PatchBadgeProps) => {
  const swatch = usePatchColor(targetKey, position);
  const background = dimmed ? color.dead : swatch.value;

  return (
    <SwatchPicker
      selected={swatch.key}
      assigned={swatch.assigned}
      onSelect={swatch.select}
      onReset={swatch.reset}
      label={label}
    >
      <Box
        fz="3xs"
        title="Choose this colour"
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
        {String(position + 1).padStart(2, '0')}
      </Box>
    </SwatchPicker>
  );
};
