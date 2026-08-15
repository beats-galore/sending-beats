import { Box } from '@mantine/core';
import { useHover } from '@mantine/hooks';

import { color } from '../../../theme/tokens';

type ExpandToggleProps = {
  expanded: boolean;
  onToggle: () => void;
};

/**
 * Condenses and expands a node.
 *
 * A node shows as much as it has room for, so this is only a shortcut: it sizes
 * the node to exactly what the state it is going to needs, which is the same
 * place dragging the corner there would land. Expanding a source with its
 * effects switched off stops at the inspector, because there is no chain below
 * it to make room for.
 */
export const ExpandToggle = ({ expanded, onToggle }: ExpandToggleProps) => {
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
      title={expanded ? 'Condense' : 'Expand'}
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
      {expanded ? '▲' : '▼'}
    </Box>
  );
};
