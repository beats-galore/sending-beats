import { Tabs, Tooltip, Stack, UnstyledButton } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo, useCallback } from 'react';
import {
  IconHome,
  IconAdjustments,
  IconMicrophone,
  IconHeadphones,
  IconSettings,
} from '@tabler/icons-react';

type ViewType = 'home' | 'dj' | 'admin' | 'listener' | 'mixer';

type NavigationProps = {
  currentView: ViewType;
  onViewChange: (view: ViewType) => void;
  collapsed?: boolean;
};

const useStyles = createStyles((theme) => ({
  navigationTabs: {
    width: '100%',
  },

  iconButton: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: 48,
    height: 48,
    borderRadius: theme.radius.md,
    backgroundColor: 'transparent',
    border: 'none',
    cursor: 'pointer',
    transition: 'background-color 0.15s ease',

    '&:hover': {
      backgroundColor: theme.colors.dark[5],
    },

    '&[data-active="true"]': {
      backgroundColor: theme.colors.blue[6],
      color: theme.white,

      '&:hover': {
        backgroundColor: theme.colors.blue[5],
      },
    },
  },

  collapsedNavigation: {
    display: 'flex',
    flexDirection: 'column',
    gap: theme.spacing.xs,
    alignItems: 'center',
  },
}));

const navigationItems = [
  { value: 'home', icon: IconHome, label: 'Home' },
  { value: 'mixer', icon: IconAdjustments, label: 'Virtual Mixer' },
  { value: 'dj', icon: IconMicrophone, label: 'DJ Client' },
  { value: 'listener', icon: IconHeadphones, label: 'Listen' },
  { value: 'admin', icon: IconSettings, label: 'Admin Panel' },
] as const;

export const Navigation = memo<NavigationProps>(
  ({ currentView, onViewChange, collapsed = true }) => {
    const { classes } = useStyles();

    const handleTabChange = useCallback(
      (value: string | null) => {
        if (value) {
          onViewChange(value as ViewType);
        }
      },
      [onViewChange]
    );

    if (collapsed) {
      return (
        <Stack className={classes.collapsedNavigation}>
          {navigationItems.map(({ value, icon: Icon, label }) => (
            <Tooltip key={value} label={label} position="right" withArrow>
              <UnstyledButton
                className={classes.iconButton}
                onClick={() => onViewChange(value as ViewType)}
                data-active={currentView === value}
              >
                <Icon size={20} />
              </UnstyledButton>
            </Tooltip>
          ))}
        </Stack>
      );
    }

    return (
      <Tabs
        value={currentView}
        onChange={handleTabChange}
        orientation="vertical"
        variant="pills"
        className={classes.navigationTabs}
      >
        <Tabs.List>
          {navigationItems.map(({ value, icon: Icon, label }) => (
            <Tabs.Tab key={value} value={value} leftSection={<Icon size={16} />}>
              {label}
            </Tabs.Tab>
          ))}
        </Tabs.List>
      </Tabs>
    );
  }
);

Navigation.displayName = 'Navigation';
