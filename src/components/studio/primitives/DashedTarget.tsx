import { Box, Stack, Text } from '@mantine/core';
import { useHover } from '@mantine/hooks';

import type { CSSProperties } from 'react';
import { layout } from '../../../theme/layout';
import { color, dashedBorder } from '../../../theme/tokens';


type DashedTargetProps = {
  label: string;
  /** Quiet second line naming what the target accepts. */
  hint?: string;
  onClick: () => void;
  height?: number;
  style?: CSSProperties;
};

/** The dashed "add one of these" placeholder that ends a column of nodes. */
export const DashedTarget = ({ label, hint, onClick, height, style }: DashedTargetProps) => {
  const { hovered, ref } = useHover();

  return (
    <Box
      ref={ref}
      onClick={onClick}
      style={{
        height,
        border: dashedBorder(hovered ? 'acc' : 'dash'),
        borderRadius: 'var(--mantine-radius-xl)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        cursor: 'pointer',
        ...style,
      }}
    >
      <Stack gap="xs" align="center">
        <Text
          fz={hint ? 'md' : 'sm'}
          fw={600}
          ff="var(--mantine-font-family-headings)"
          c={hovered ? color.acc : color.textFaint}
          style={{ letterSpacing: layout.tracking.wide }}
        >
          {label}
        </Text>
        {hint && (
          <Text size="2xs" c={color.textFaintest}>
            {hint}
          </Text>
        )}
      </Stack>
    </Box>
  );
};
