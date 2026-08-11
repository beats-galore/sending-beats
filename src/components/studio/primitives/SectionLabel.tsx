import { Text } from '@mantine/core';

import type { MantineStyleProps } from '@mantine/core';
import type { CSSProperties, ReactNode } from 'react';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';


type SectionLabelProps = {
  children: ReactNode;
  /** How far the capitals are tracked out. Wider reads as a higher-level heading. */
  tracking?: keyof typeof layout.tracking;
  tone?: 'faint' | 'muted' | 'dim';
  style?: CSSProperties;
} & MantineStyleProps;

const TONES = {
  faint: color.textFaint,
  muted: color.textMuted,
  dim: color.textDim,
} as const;

/**
 * The tracked-out capital label that titles a region inside a panel.
 * Deliberately quiet — it names a group without competing with its contents.
 */
export const SectionLabel = ({
  children,
  tracking = 'heading',
  tone = 'faint',
  style,
  ...styleProps
}: SectionLabelProps) => (
  <Text
    size="2xs"
    c={TONES[tone]}
    style={{ letterSpacing: layout.tracking[tracking], whiteSpace: 'nowrap', ...style }}
    {...styleProps}
  >
    {children}
  </Text>
);
