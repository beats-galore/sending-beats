import { Box, Group, Slider, ActionIcon, Stack, Text, Tooltip } from '@mantine/core';
import { IconVolume2, IconVolume3, IconX } from '@tabler/icons-react';
import { createStyles } from '@mantine/styles';
import { memo, useCallback, useMemo } from 'react';

import { useChannelLevels } from '../../../hooks/use-channel-levels';
import { useMixerStore } from '../../../stores';
import type { ProcessInfo } from '../../../types/applicationAudio.types';

const useStyles = createStyles((theme) => ({
  container: {
    backgroundColor: theme.colors.dark[7],
    borderRadius: theme.radius.md,
    padding: theme.spacing.sm,
    border: `1px solid ${theme.colors.dark[5]}`,
  },

  appName: {
    fontSize: theme.fontSizes.sm,
    fontWeight: 500,
    color: theme.colors.gray[0],
  },

  controls: {
    flex: 1,
  },

  slider: {
    flex: 1,
  },

  levelBadge: {
    fontSize: '0.75rem',
    color: theme.colors.gray[4],
    minWidth: 30,
    textAlign: 'right',
  },

  volumeIcon: {
    color: theme.colors.blue[4],
  },

  muteButton: {
    '&:hover': {
      backgroundColor: theme.colors.dark[5],
    },
  },

  closeButton: {
    color: theme.colors.red[5],
    '&:hover': {
      backgroundColor: theme.colors.dark[5],
    },
  },
}));

type ApplicationVolumeControlProps = {
  app: ProcessInfo;
  onStop: () => Promise<boolean>;
};

export const ApplicationVolumeControl = memo(
  ({ app, onStop }: ApplicationVolumeControlProps) => {
    const { classes } = useStyles();

    // Find the channel index for this app (format is app-{pid})
    const config = useMixerStore((state) => state.config);
    const updateChannel = useMixerStore((state) => state.updateChannel);

    const channelIndex = useMemo(() => {
      if (!config) return -1;
      return config.channels.findIndex(
        (ch) => ch.name && ch.name.includes(app.name)
      );
    }, [config, app.name]);

    const channel = useMemo(() => {
      if (channelIndex === -1 || !config) return null;
      return config.channels[channelIndex];
    }, [channelIndex, config]);

    const channelLevels = useChannelLevels(channelIndex);

    const handleVolumeChange = useCallback(
      (value: number) => {
        if (channelIndex === -1) return;
        void updateChannel(channelIndex, { channel_gain: value / 100 });
      },
      [channelIndex, updateChannel]
    );

    const handleMuteToggle = useCallback(() => {
      if (!channel) return;
      void updateChannel(channelIndex, { channel_mute: !channel.channel_mute });
    }, [channel, channelIndex, updateChannel]);

    const handleStop = useCallback(async () => {
      await onStop();
    }, [onStop]);

    if (!channel) {
      return null;
    }

    const peakDb = channelLevels.peak || -Infinity;
    const peakColor =
      peakDb > -6 ? 'red' : peakDb > -12 ? 'yellow' : 'green';

    return (
      <Box className={classes.container}>
        <Group justify="space-between" align="flex-start" wrap="nowrap">
          <Stack gap={4} style={{ flex: 1 }}>
            <Text className={classes.appName}>{app.name}</Text>

            <Group gap="xs" align="center" className={classes.controls}>
              <Tooltip label={channel.channel_mute ? 'Unmute' : 'Mute'} withArrow>
                <ActionIcon
                  size="sm"
                  variant="subtle"
                  onClick={handleMuteToggle}
                  className={classes.muteButton}
                >
                  {channel.channel_mute ? (
                    <IconVolume3 size={16} className={classes.volumeIcon} />
                  ) : (
                    <IconVolume2 size={16} className={classes.volumeIcon} />
                  )}
                </ActionIcon>
              </Tooltip>

              <Slider
                className={classes.slider}
                value={channel.channel_gain * 100}
                onChange={handleVolumeChange}
                min={0}
                max={100}
                step={1}
                disabled={channel.channel_mute}
                color={channel.channel_mute ? 'gray' : 'blue'}
              />

              <Text className={classes.levelBadge} c={peakColor}>
                {peakDb.toFixed(1)}dB
              </Text>

              <Tooltip label="Stop capturing" withArrow>
                <ActionIcon
                  size="sm"
                  variant="subtle"
                  onClick={handleStop}
                  className={classes.closeButton}
                >
                  <IconX size={16} />
                </ActionIcon>
              </Tooltip>
            </Group>
          </Stack>
        </Group>
      </Box>
    );
  }
);

ApplicationVolumeControl.displayName = 'ApplicationVolumeControl';
