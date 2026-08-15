import { Box, Text } from '@mantine/core';
import { IconTrash } from '@tabler/icons-react';
import { useEffect, useRef, useState } from 'react';

import type { MouseEvent } from 'react';
import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';


/** How long the armed state waits for the second click before giving up. */
const ARM_TIMEOUT_MS = 3000;

type DeleteButtonProps = {
  onDelete: () => void;
  /** Describes what is being deleted, for the tooltip on the resting state. */
  title: string;
};

/**
 * A trash icon that expands into its own confirmation rather than opening a
 * modal: the first click arms it, the second deletes. Disarms on a timeout so a
 * stray click cannot leave a primed delete sitting on the canvas.
 */
export const DeleteButton = ({ onDelete, title }: DeleteButtonProps) => {
  const [armed, setArmed] = useState(false);
  const timeout = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!armed) {
      return;
    }
    timeout.current = setTimeout(() => setArmed(false), ARM_TIMEOUT_MS);
    return () => {
      if (timeout.current) {
        clearTimeout(timeout.current);
      }
    };
  }, [armed]);

  const handleClick = (event: MouseEvent<HTMLDivElement>) => {
    // The whole card is a click target that selects the node, so neither the
    // arming click nor the confirming one may reach it.
    event.stopPropagation();

    if (!armed) {
      setArmed(true);
      return;
    }

    setArmed(false);
    onDelete();
  };

  return (
    <Box
      onClick={handleClick}
      // Lives in a node's title bar, which is also the grip that moves the node
      data-no-drag
      title={armed ? 'Click again to confirm' : title}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 4,
        flex: 'none',
        padding: armed ? '2px 6px' : 2,
        borderRadius: 'var(--mantine-radius-sm)',
        border: `1px solid ${armed ? color.hot : 'transparent'}`,
        background: armed ? color.hotBg : undefined,
        color: armed ? color.hotText : color.textFaint,
        cursor: 'pointer',
      }}
    >
      <IconTrash size={12} stroke={1.5} />
      {armed ? (
        <Text size="3xs" fw={600} style={{ letterSpacing: layout.tracking.wide }}>
          SURE?
        </Text>
      ) : null}
    </Box>
  );
};
