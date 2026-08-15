import { Box, Group } from '@mantine/core';

import type { CSSProperties, MouseEventHandler, PointerEventHandler, ReactNode } from 'react';
import { color } from '../../../theme/tokens';
import type { ColorToken } from '../../../theme/tokens';
import { ResizeGrip } from './ResizeGrip';


type NodeCardProps = {
  /** Contents of the title bar. */
  header: ReactNode;
  children?: ReactNode;
  /** Absolute placement inside the patch canvas. */
  position: { left: number; top: number; width: number; height?: number };
  borderColor?: string;
  headerSurface?: ColorToken;
  /** Rings the node, for the selected channel. */
  selected?: boolean;
  /** Draws the node above every other, for the one last pressed. */
  raised?: boolean;
  /** Called on any press, to bring the node forward out of a stack. */
  onPress?: PointerEventHandler<HTMLDivElement>;
  onClick?: MouseEventHandler<HTMLDivElement>;
  /** Picks the node up. The title bar is the grip. */
  onGrab?: PointerEventHandler<HTMLDivElement>;
  /** Resizes the node. Draws the corner grip when given. */
  onResize?: PointerEventHandler<HTMLDivElement>;
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
  raised = false,
  onPress,
  onClick,
  onGrab,
  onResize,
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
    onPointerDown={onPress}
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
      zIndex: raised ? 20 : selected ? 5 : undefined,
      boxShadow: selected ? `0 0 0 4px ${color.accDim}` : undefined,
    }}
  >
    <Group
      h={HEADER_HEIGHT}
      px="lg"
      gap="sm"
      wrap="nowrap"
      onPointerDown={onGrab}
      style={{
        flex: 'none',
        borderBottom: `1px solid ${color.line}`,
        background: color[headerSurface],
        borderRadius: 'var(--mantine-radius-lg) var(--mantine-radius-lg) 0 0',
        cursor: onGrab ? 'grab' : undefined,
        // The title is the grip. Selecting it instead of dragging by it is
        // never what was meant.
        userSelect: onGrab ? 'none' : undefined,
      }}
    >
      {header}
    </Group>

    {children && (
      <Box style={{ flex: 1, minHeight: 0, padding: '10px 11px', ...bodyStyle }}>{children}</Box>
    )}

    {/* The grip holds its press back from the node, so bringing the node
        forward has to be asked for here rather than left to bubble. */}
    {onResize && (
      <ResizeGrip
        onResize={(event) => {
          onPress?.(event);
          onResize(event);
        }}
      />
    )}
    {ports}
  </Box>
);
