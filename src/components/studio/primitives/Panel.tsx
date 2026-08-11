import { Group, Paper, Stack } from '@mantine/core';


import type { MantineSpacing, PaperProps } from '@mantine/core';
import type { ReactNode } from 'react';
import { PanelHeading } from './PanelHeading';

type PanelProps = {
  /** Rendered as a condensed heading plate above the contents. */
  title?: ReactNode;
  /** Placed at the far end of the title row — a link, count or control. */
  action?: ReactNode;
  gap?: MantineSpacing;
  children: ReactNode;
} & PaperProps;

/**
 * The standard content surface: a bordered panel with an optional heading row.
 * Background, border and radius come from the Paper defaults in the theme.
 */
export const Panel = ({
  title,
  action,
  gap = 'lg',
  children,
  p = '3xl',
  ...paperProps
}: PanelProps) => (
  <Paper p={p} {...paperProps}>
    <Stack gap={gap} h="100%">
      {(title || action) && (
        <Group gap="md" wrap="nowrap">
          {typeof title === 'string' ? <PanelHeading>{title}</PanelHeading> : title}
          {action && (
            <>
              <div style={{ flex: 1 }} />
              {action}
            </>
          )}
        </Group>
      )}
      {children}
    </Stack>
  </Paper>
);
