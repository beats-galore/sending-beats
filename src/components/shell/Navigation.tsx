import { Tabs } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo, useCallback } from 'react';
import { IconHome, IconAdjustments, IconMicrophone, IconHeadphones, IconSettings } from '@tabler/icons-react';

type ViewType = 'home' | 'dj' | 'admin' | 'listener' | 'mixer';

type NavigationProps = {
  currentView: ViewType;
  onViewChange: (view: ViewType) => void;
};

const useStyles = createStyles(() => ({
  navigationTabs: {
    width: '100%',
  },
}));

export const Navigation = memo<NavigationProps>(({ currentView, onViewChange }) => {
  const { classes } = useStyles();
  
  const handleTabChange = useCallback((value: string | null) => {
    if (value) {
      onViewChange(value as ViewType);
    }
  }, [onViewChange]);
  
  return (
    <Tabs
      value={currentView}
      onChange={handleTabChange}
      orientation="vertical"
      variant="pills"
      className={classes.navigationTabs}
    >
      <Tabs.List>
        <Tabs.Tab
          value="home"
          leftSection={<IconHome size={16} />}
        >
          Home
        </Tabs.Tab>
        <Tabs.Tab
          value="mixer"
          leftSection={<IconAdjustments size={16} />}
        >
          Virtual Mixer
        </Tabs.Tab>
        <Tabs.Tab
          value="dj"
          leftSection={<IconMicrophone size={16} />}
        >
          DJ Client
        </Tabs.Tab>
        <Tabs.Tab
          value="listener"
          leftSection={<IconHeadphones size={16} />}
        >
          Listen
        </Tabs.Tab>
        <Tabs.Tab
          value="admin"
          leftSection={<IconSettings size={16} />}
        >
          Admin Panel
        </Tabs.Tab>
      </Tabs.List>
    </Tabs>
  );
});

Navigation.displayName = 'Navigation';