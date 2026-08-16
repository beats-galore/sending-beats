import { Box, Group, Select, Text } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { useMemo } from 'react';

import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import type { SourceOption } from '../hooks/use-add-source';
import { ActionButton } from '../primitives/ActionButton';

type AddTileProps = {
  label: string;
  /** What this tile can add, including what is already on the patch. */
  options: SourceOption[];
  /** What to say when there is nothing of this kind at all. */
  emptyHint: string;
  onPick: (value: string) => void;
};

const SELECT_WIDTH = 260;

/**
 * One kind of thing you can add, and the search for which one.
 *
 * The tile becomes the picker in place rather than opening a menu beside it: a
 * dock of tiles that each spawn a floating panel is a dock you have to aim at.
 *
 * A searchable Mantine select rather than the platform's own menu — a machine
 * with a dozen virtual devices makes a native menu a scrolling wall with no way
 * to type at it.
 */
export const AddTile = ({ label, options, emptyHint, onPick }: AddTileProps) => {
  const [picking, { open, close }] = useDisclosure(false);

  // What is already patched sinks to the bottom, still listed. A device missing
  // from the list reads as a device that has gone away; one sitting at the
  // bottom marked with the strip it is on answers the question instead.
  const ordered = useMemo(() => {
    const free = options.filter((option) => !option.patchedAt);
    const taken = options.filter((option) => option.patchedAt);
    return [...free, ...taken];
  }, [options]);

  const anyFree = ordered.some((option) => !option.patchedAt);

  if (!picking) {
    return (
      <ActionButton onClick={options.length > 0 ? open : undefined} disabled={options.length === 0}>
        {label}
      </ActionButton>
    );
  }

  return (
    <Select
      data={ordered.map((option) => ({
        value: option.value,
        label: option.label,
        disabled: option.patchedAt !== undefined,
      }))}
      placeholder={anyFree ? 'Search…' : emptyHint}
      searchable
      autoFocus
      defaultDropdownOpened
      comboboxProps={{ withinPortal: true }}
      // Closing the dropdown is what puts the tile back: clicking away, Escape
      // and choosing all close it, where watching the input lose focus races
      // with the click that opened it in the first place.
      onDropdownClose={close}
      maxDropdownHeight={300}
      w={SELECT_WIDTH}
      size="xs"
      nothingFoundMessage="Nothing matches"
      onChange={(value) => {
        if (value) {
          onPick(value);
        }
      }}
      renderOption={({ option }) => {
        const entry = ordered.find((candidate) => candidate.value === option.value);

        return (
          <Group gap="xs" wrap="nowrap" style={{ flex: 1, minWidth: 0 }}>
            <Box style={{ flex: 1, minWidth: 0 }}>
              <Text size="xs" truncate>
                {option.label}
              </Text>
              {entry?.detail && (
                <Text size="3xs" c={color.textFaintest} truncate>
                  {entry.detail}
                </Text>
              )}
            </Box>
            {entry?.patchedAt && <PatchedBadge at={entry.patchedAt} />}
          </Group>
        );
      }}
      styles={{
        input: {
          background: color.bg,
          border: border(),
          letterSpacing: layout.tracking.tight,
        },
        // The dropdown sits over the canvas, which is nearly black. Left to
        // inherit it reads as a smudge rather than a list.
        dropdown: { background: color.panel, border: border('lineStrong') },
        option: { paddingTop: 4, paddingBottom: 4, color: color.text },
      }}
    />
  );
};

/** Which strip a thing is already on, in that strip's own colour. */
const PatchedBadge = ({ at }: { at: NonNullable<SourceOption['patchedAt']> }) => (
  <Box
    fz="3xs"
    style={{
      flex: 'none',
      padding: '1px 5px',
      borderRadius: 'var(--mantine-radius-2xs)',
      fontWeight: 600,
      letterSpacing: layout.tracking.wide,
      color: at.color,
      border: `1px solid ${at.color}`,
    }}
  >
    {String(at.position + 1).padStart(2, '0')}
  </Box>
);
