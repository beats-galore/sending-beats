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
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
    textAlign: 'center',
  },
}));

export const StatCard = memo<StatCardProps>(({ icon, value, label, color = 'blue' }) => {
  const { classes } = useStyles();

  return (
    <Card className={classes.statCard} padding="lg" withBorder>
      <Stack align="center" gap="sm">
        <ThemeIcon size={40} color={color} variant="light">
          {icon}
        </ThemeIcon>
        <Text size="xl" fw={700} c={`${color}.4`}>
          {value}
        </Text>
        <Text size="sm" c="dimmed">
          {label}
        </Text>
      </Stack>
    </Card>
  );
});

StatCard.displayName = 'StatCard';
