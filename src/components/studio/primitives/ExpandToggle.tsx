import { Box } from '@mantine/core';
import { useHover } from '@mantine/hooks';

import { color } from '../../../theme/tokens';

type ExpandToggleProps = {
  /** Whether the next press makes the node bigger or folds it back down. */
  grows: boolean;
  onToggle: () => void;
};

/**
 * Steps a node through its sizes.
 *
 * A node shows as much as it has room for, so this is only a shortcut: it sizes
 * the node to exactly what the next state needs, which is the same place
 * dragging the corner there would land.
 *
 * There are more than two states now — shrunk to one reading, shut, and open —
 * so rather than claiming a node is simply open or shut, the arrow says which
 * way the next press goes. It walks up through the sizes and folds all the way
 * back down from the top.
 */
export const ExpandToggle = ({ grows, onToggle }: ExpandToggleProps) => {
  const { hovered, ref } = useHover();

  return (
    <Box
      ref={ref}
      onClick={(event) => {
        event.stopPropagation();
        onToggle();
      }}
      // Lives in the title bar, which is also the grip that moves the node
      data-no-drag
      title={grows ? 'Show more' : 'Shrink to a single line'}
      style={{
        width: 16,
        height: 16,
        flex: 'none',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        borderRadius: 'var(--mantine-radius-xs)',
        border: `1px solid ${hovered ? color.acc : color.line}`,
        color: hovered ? color.acc : color.textFaint,
        cursor: 'pointer',
        fontSize: 9,
        lineHeight: 1,
      }}
    >
      {grows ? '▼' : '▲'}
    </Box>
  );
};
