import { Box, Group, Stack, Text, TextInput } from '@mantine/core';

import { selectedQueue, useQueueStore } from '../../../stores/queue-store';
import { layout } from '../../../theme/layout';
import { border, color } from '../../../theme/tokens';
import { ActionButton } from '../primitives/ActionButton';
import { DeleteButton } from '../primitives/DeleteButton';
import { Panel } from '../primitives/Panel';
import { SectionLabel } from '../primitives/SectionLabel';

/**
 * Which queue is being looked at, and the making of new ones.
 *
 * A queue belongs to the studio rather than to a patch, so this is the whole
 * list of them — the same shape the station picker has, for the same reason:
 * you build the thing once and point patches at it afterwards.
 */
export const QueuePicker = () => {
  const queues = useQueueStore((state) => state.queues);
  const targetIds = useQueueStore((state) => state.targetIds);
  const selected = useQueueStore(selectedQueue);
  const select = useQueueStore((state) => state.select);
  const add = useQueueStore((state) => state.add);
  const rename = useQueueStore((state) => state.rename);
  const remove = useQueueStore((state) => state.remove);

  return (
    <Panel>
      <Group justify="space-between" wrap="nowrap">
        <SectionLabel>QUEUES</SectionLabel>
        <ActionButton onClick={() => void add()}>+ NEW QUEUE</ActionButton>
      </Group>

      <Stack gap="3xs" mt="lg">
        {queues.length === 0 ? (
          <Text size="xs" c={color.textFaint} ta="center" py="xl">
            No queues yet. Make one and drop files into it.
          </Text>
        ) : (
          queues.map((queue) => {
            const active = queue.id === selected?.id;

            return (
              <Group
                key={queue.id}
                onClick={() => void select(queue.id)}
                gap="sm"
                wrap="nowrap"
                px="md"
                py="sm"
                style={{
                  borderRadius: 'var(--mantine-radius-sm)',
                  cursor: 'pointer',
                  background: active ? color.panelHi : undefined,
                  border: active ? border('acc') : `1px solid transparent`,
                }}
              >
                <Text size="sm" truncate style={{ flex: 1 }}>
                  {queue.name}
                </Text>
                {/* Says this queue is on the patch you have open, which is the
                    difference between a list you are editing and one that is
                    about to go out. */}
                {targetIds.includes(queue.id) && (
                  <Text
                    size="3xs"
                    c={color.acc}
                    style={{ flex: 'none', letterSpacing: layout.tracking.wide }}
                  >
                    ON PATCH
                  </Text>
                )}
              </Group>
            );
          })
        )}
      </Stack>

      {selected && (
        <Box mt="xl" pt="lg" style={{ borderTop: border() }}>
          <SectionLabel>NAME</SectionLabel>
          <Group gap="sm" wrap="nowrap" mt="sm">
            <TextInput
              value={selected.name}
              onChange={(event) => void rename(selected.id, event.currentTarget.value)}
              size="xs"
              style={{ flex: 1 }}
            />
            <DeleteButton
              onDelete={() => void remove(selected.id)}
              title={`Delete ${selected.name} and everything in it`}
            />
          </Group>
        </Box>
      )}
    </Panel>
  );
};
