import { Card, Stack, Title, TextInput, Grid, Select, Group, Button } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconWifi, IconWifiOff } from '@tabler/icons-react';
import { memo, useEffect, useState } from 'react';

type StreamSettings = {
  bitrate: number;
  sampleRate: number;
  channels: number;
};

type StreamConfig = {
  icecast_url: string;
  mount_point: string;
  username: string;
  password: string;
  bitrate: number;
  sample_rate: number;
  channels: number;
};

type StreamConfigurationCardProps = {
  streamConfig: StreamConfig;
  streamSettings: StreamSettings;
  isConnected: boolean;
  isConnecting: boolean;
  availableBitrates?: number[];
  onConfigChange: (config: StreamConfig) => void;
  onSettingsChange: (settings: StreamSettings) => void;
  onConnect: () => void;
  onDisconnect: () => void;
};

const useStyles = createStyles((theme) => ({
  configCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },
}));

export const StreamConfigurationCard = memo<StreamConfigurationCardProps>(
  ({
    streamConfig,
    streamSettings,
    isConnected,
    isConnecting,
    availableBitrates = [96, 128, 160, 192, 256, 320],
    onConfigChange,
    onSettingsChange,
    onConnect,
    onDisconnect,
  }) => {
    const { classes } = useStyles();
    
    // Generate bitrate options from available bitrates
    const bitrateOptions = availableBitrates.map(bitrate => ({
      value: bitrate.toString(),
      label: `${bitrate} kbps`
    }));

    return (
      <Card className={classes.configCard} padding="lg" withBorder>
        <Stack gap="md">
          <Title order={3} c="blue.4">
            Stream Configuration
          </Title>

          <TextInput
            label="Icecast URL"
            placeholder="http://localhost:8000"
            value={streamConfig.icecast_url}
            onChange={(e) => onConfigChange({ ...streamConfig, icecast_url: e.target.value })}
          />

          <TextInput
            label="Mount Point"
            placeholder="live"
            value={streamConfig.mount_point}
            onChange={(e) => onConfigChange({ ...streamConfig, mount_point: e.target.value })}
          />

          <Grid>
            <Grid.Col span={6}>
              <TextInput
                label="Username"
                placeholder="source"
                value={streamConfig.username}
                onChange={(e) => onConfigChange({ ...streamConfig, username: e.target.value })}
              />
            </Grid.Col>
            <Grid.Col span={6}>
              <TextInput
                type="password"
                label="Password"
                placeholder="Enter Icecast password"
                value={streamConfig.password}
                onChange={(e) => onConfigChange({ ...streamConfig, password: e.target.value })}
              />
            </Grid.Col>
          </Grid>

          <Select
            label="Bitrate (kbps)"
            description="Restart streaming to apply changes"
            value={streamSettings.bitrate.toString()}
            onChange={(value) => onSettingsChange({ ...streamSettings, bitrate: Number(value) })}
            data={bitrateOptions}
            disabled={isConnected}
          />

          <Group grow>
            {!isConnected ? (
              <Button
                onClick={onConnect}
                leftSection={<IconWifi size={16} />}
                loading={isConnecting}
                color="blue"
              >
                Connect
              </Button>
            ) : (
              <Button
                onClick={onDisconnect}
                leftSection={<IconWifiOff size={16} />}
                color="red"
                variant="light"
              >
                Disconnect
              </Button>
            )}
          </Group>
        </Stack>
      </Card>
    );
  }
);

StreamConfigurationCard.displayName = 'StreamConfigurationCard';
