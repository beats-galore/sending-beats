import { Box, Group } from '@mantine/core';

import type { CSSProperties, MouseEventHandler, ReactNode } from 'react';
import { color } from '../../../theme/tokens';

import type { ColorToken } from '../../../theme/tokens';

type NodeCardProps = {
  /** Contents of the title bar. */
  header: ReactNode;
  children?: ReactNode;
  /** Absolute placement inside the patch canvas. */
  position: { left: number; top: number; width: number; height?: number };
  borderColor?: string;
  headerSurface?: ColorToken;
  /** Lifts the node above its neighbours and rings it, for the selected channel. */
  selected?: boolean;
  onClick?: MouseEventHandler<HTMLDivElement>;
  dimmed?: boolean;
  /** Ports, which sit outside the card's own bounds. */
  ports?: ReactNode;
  bodyStyle?: CSSProperties;
};

const HEADER_HEIGHT = 30;

/**
 * A node on the patch canvas: a titled card at a fixed position with patch
 * points on its edges. Shared by channels, destinations and the tape.
 */
export const NodeCard = ({
  header,
  children,
  position,
  borderColor = color.line,
  headerSurface = 'bgRaised',
  selected = false,
  onClick,
  dimmed = false,
  ports,
  bodyStyle,
}: NodeCardProps) => (
  <Box
    // Held inside the node: the canvas clears the selection on any click that
    // reaches it, and a click on a node is not a click on the canvas.
    onClick={
      onClick &&
      ((event) => {
        event.stopPropagation();
        onClick(event);
      })
    }
    style={{
      position: 'absolute',
      left: position.left,
      top: position.top,
      width: position.width,
      height: position.height,
      background: color.panel,
      border: `1px solid ${borderColor}`,
      borderRadius: 'var(--mantine-radius-xl)',
      display: 'flex',
      flexDirection: 'column',
      cursor: onClick ? 'pointer' : undefined,
      opacity: dimmed ? 0.6 : 1,
      zIndex: selected ? 5 : undefined,
      boxShadow: selected ? `0 0 0 4px ${color.accDim}` : undefined,
    }}
  >
    <Group
      h={HEADER_HEIGHT}
      px="lg"
      gap="sm"
      wrap="nowrap"
      style={{
        flex: 'none',
        borderBottom: `1px solid ${color.line}`,
        background: color[headerSurface],
        borderRadius: 'var(--mantine-radius-lg) var(--mantine-radius-lg) 0 0',
      }}
    >
      {header}
    </Group>

    {children && (
      <Box style={{ flex: 1, minHeight: 0, padding: '10px 11px', ...bodyStyle }}>{children}</Box>
    )}

    {ports}
  </Box>
);
