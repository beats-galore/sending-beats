import { AppShell, Text } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';

const useStyles = createStyles((theme) => ({
  footer: {
    backgroundColor: theme.colors.dark[7],
    borderTop: `1px solid ${theme.colors.dark[5]}`,
    padding: '8px 16px',
  },
  
  footerText: {
    textAlign: 'center',
  },
}));

export const AppFooter = memo(() => {
  const { classes } = useStyles();
  
  return (
    <AppShell.Footer className={classes.footer}>
      <Text size="xs" c="dimmed" className={classes.footerText}>
        &copy; {new Date().getFullYear()} Sendin Beats - Professional Radio Streaming Platform
      </Text>
    </AppShell.Footer>
  );
});

AppFooter.displayName = 'AppFooter';