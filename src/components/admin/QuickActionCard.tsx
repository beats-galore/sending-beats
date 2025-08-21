import { Card, Stack, Box, Text, ThemeIcon } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';

type QuickActionCardProps = {
  icon: React.ReactNode;
  title: string;
  description: string;
  onClick?: () => void;
};

const useStyles = createStyles((theme) => ({
  quickActionCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
    cursor: 'pointer',
    transition: 'all 0.2s ease',

    '&:hover': {
      backgroundColor: theme.colors.dark[5],
      transform: 'translateY(-2px)',
    },
  },

  cardContent: {
    textAlign: 'center',
  },
}));

export const QuickActionCard = memo<QuickActionCardProps>(
  ({ icon, title, description, onClick }) => {
    const { classes } = useStyles();

    return (
      <Card className={classes.quickActionCard} padding="lg" withBorder onClick={onClick}>
        <Stack align="center" gap="md">
          <ThemeIcon size={50} color="blue" variant="light">
            {icon}
          </ThemeIcon>
          <Box className={classes.cardContent}>
            <Text fw={600} mb={4}>
              {title}
            </Text>
            <Text size="sm" c="dimmed">
              {description}
            </Text>
          </Box>
        </Stack>
      </Card>
    );
  }
);

QuickActionCard.displayName = 'QuickActionCard';
