import { Center, Stack, Text } from '@mantine/core';

import { color } from '../../../theme/tokens';
import { ActionButton } from '../primitives/ActionButton';
import { Panel } from '../primitives/Panel';
import { StatusDot } from '../primitives/StatusDot';

type StudioErrorProps = {
  title: string;
  message: string;
  onRetry?: () => void;
};

/** A blocking failure, shown in place of the view that cannot render. */
export const StudioError = ({ title, message, onRetry }: StudioErrorProps) => (
  <Center style={{ flex: 1 }} p="5xl">
    <Panel
      maw={440}
      title={
        <Stack gap="sm">
          <StatusDot tone="hot" size={8} />
          <Text ff="var(--mantine-font-family-headings)" fw={700} fz="xl" c={color.hotText}>
            {title}
          </Text>
        </Stack>
      }
      style={{ borderColor: color.hotBorder }}
    >
      <Text size="sm" c={color.textDim} lh="lg">
        {message}
      </Text>
      {onRetry && (
        <ActionButton tone="ghost" onClick={onRetry}>
          RETRY
        </ActionButton>
      )}
    </Panel>
  </Center>
);
