import { Group, Text } from '@mantine/core';


import type { MantineFontSize } from '@mantine/core';
import type { MouseEventHandler, ReactNode } from 'react';
import { color } from '../../../theme/tokens';

type StatRowProps = {
  label: ReactNode;
  children: ReactNode;
  /** Colours the reading — used to flag a value that is near or over a limit. */
  tone?: string;
  size?: MantineFontSize;
  onClick?: MouseEventHandler<HTMLDivElement>;
};

/** A label on the left, its reading on the right. The workhorse of every readout block. */
export const StatRow = ({ label, children, tone, size = 'xs', onClick }: StatRowProps) => (
  <Group
    justify="space-between"
    gap="sm"
    wrap="nowrap"
    onClick={onClick}
    style={{ cursor: onClick ? 'pointer' : undefined }}
  >
    <Text size={size} c={color.textFaint}>
      {label}
    </Text>
    {typeof children === 'string' || typeof children === 'number' ? (
      <Text size={size} c={tone ?? color.textDim}>
        {children}
      </Text>
    ) : (
      children
    )}
  </Group>
);
