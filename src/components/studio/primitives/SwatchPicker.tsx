import { Box, Group, Popover, Stack, Text } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';

import type { ReactNode } from 'react';
import { border, color, Swatch, swatchColor } from '../../../theme/tokens';
import { SectionLabel } from './SectionLabel';

type SwatchPickerProps = {
  /** What opens the picker — the thing being coloured */
  children: ReactNode;
  selected: Swatch;
  /** False while `selected` is only the derived fallback */
  assigned: boolean;
  onSelect: (key: Swatch) => void;
  onReset: () => void;
  label: string;
};

/** Picks the colour a signal is drawn in, wherever it appears on the patchbay. */
export const SwatchPicker = ({
  children,
  selected,
  assigned,
  onSelect,
  onReset,
  label,
}: SwatchPickerProps) => {
  const [opened, { toggle, close }] = useDisclosure(false);

  return (
    <Popover opened={opened} onChange={close} position="bottom-start" offset={6} width={168}>
      <Popover.Target>
        <Box
          onClick={(event) => {
            // The badge sits inside a card that selects on click, which would
            // expand the node behind the picker the moment it opened.
            event.stopPropagation();
            toggle();
          }}
          style={{ flex: 'none', cursor: 'pointer' }}
        >
          {children}
        </Box>
      </Popover.Target>

      <Popover.Dropdown p="sm" onClick={(event) => event.stopPropagation()}>
        <Stack gap="sm">
          <SectionLabel>{label}</SectionLabel>

          <Group gap="3xs" wrap="nowrap">
            {Swatch.map((key) => (
              <Box
                key={key}
                onClick={() => {
                  onSelect(key);
                  close();
                }}
                title={key}
                style={{
                  flex: 1,
                  height: 22,
                  borderRadius: 'var(--mantine-radius-xs)',
                  cursor: 'pointer',
                  background: swatchColor(key),
                  // Only a deliberate choice is marked. Showing the ring on the
                  // derived fallback would read as already picked.
                  outline: assigned && key === selected ? `2px solid ${color.text}` : undefined,
                  outlineOffset: 1,
                }}
              />
            ))}
          </Group>

          <Text
            size="3xs"
            c={assigned ? color.textDim : color.textFaintest}
            onClick={
              assigned
                ? () => {
                    onReset();
                    close();
                  }
                : undefined
            }
            style={{
              cursor: assigned ? 'pointer' : 'default',
              borderTop: border(),
              paddingTop: 6,
            }}
          >
            {assigned ? 'Reset to automatic' : 'Automatic'}
          </Text>
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
};
