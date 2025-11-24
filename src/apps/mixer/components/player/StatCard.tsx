import { Card, Stack, Text, ThemeIcon } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';

type StatCardProps = {
  icon: React.ReactNode;
  value: string | number;
  label: string;
  color?: string;
};

const useStyles = createStyles((theme) => ({
  statCard: {
    backgroundColor: theme.colors.dark[7],
    border: `1px solid ${theme.colors.dark[5]}`,
    textAlign: 'center',
    padding: theme.spacing.md,
  },
}));

export const StatCard = memo<StatCardProps>(({ icon, value, label, color = 'blue' }) => {
  const { classes } = useStyles();

  return (
    <Card className={classes.statCard} withBorder>
      <Stack align="center" gap="sm">
        <ThemeIcon size={32} color={color} variant="light">
          {icon}
        </ThemeIcon>
        <Text size="lg" fw={700} c={`${color}.4`}>
          {value}
        </Text>
        <Text size="xs" c="dimmed">
          {label}
        </Text>
      </Stack>
    </Card>
  );
});

StatCard.displayName = 'StatCard';
