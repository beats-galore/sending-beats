// Enhanced device selector that includes both hardware devices and application sources
import { Group, Select, ActionIcon, Button, Text, Loader, Alert } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconRefresh, IconApps, IconDevices, IconAlertCircle } from '@tabler/icons-react';
import { memo, useCallback, useMemo, useEffect } from 'react';

import { useApplicationAudio } from '../../hooks';
import type { AudioDeviceInfo } from '../../types';
import type { AudioSource, AudioSourceGroup, ApplicationAudioSource, HardwareAudioSource } from '../../types/applicationAudio.types';
import { createApplicationSource, createHardwareSource, groupAudioSources } from '../../types/applicationAudio.types';

const useStyles = createStyles((theme) => ({
  selectFlex: {
    flex: 1,
  },
  
  groupLabel: {
    fontSize: theme.fontSizes.xs,
    fontWeight: 600,
    color: theme.colors.gray[4],
    textTransform: 'uppercase',
    letterSpacing: '0.05em',
    padding: `${theme.spacing.xs}px ${theme.spacing.md}px`,
    backgroundColor: theme.colors.dark[6],
    borderBottom: `1px solid ${theme.colors.dark[4]}`,
  },
  
  appOption: {
    display: 'flex',
    alignItems: 'center',
    gap: theme.spacing.xs,
    padding: `${theme.spacing.xs}px ${theme.spacing.md}px`,
    
    '&:hover': {
      backgroundColor: theme.colors.dark[5],
    },
  },
  
  appIcon: {
    width: 16,
    height: 16,
    color: theme.colors.blue[4],
  },
  
  deviceIcon: {
    width: 16,
    height: 16,
    color: theme.colors.green[4],
  },
  
  permissionAlert: {
    marginTop: theme.spacing.xs,
  },
  
  statusText: {
    fontSize: theme.fontSizes.xs,
    color: theme.colors.gray[5],
  },
}));

type EnhancedDeviceSelectorProps = {
  inputDevices: AudioDeviceInfo[];
  selectedDeviceId: string | null;
  onInputDeviceChange: (deviceId: string | null) => void;
  onRefreshDevices: () => void;
  disabled?: boolean;
};

export const EnhancedDeviceSelector = memo<EnhancedDeviceSelectorProps>(({
  inputDevices,
  selectedDeviceId,
  onInputDeviceChange,
  onRefreshDevices,
  disabled = false,
}) => {
  const { classes } = useStyles();
  const applicationAudio = useApplicationAudio();

  // Create combined audio sources
  const audioSources = useMemo((): AudioSource[] => {
    const hardwareSources: HardwareAudioSource[] = inputDevices.map(createHardwareSource);
    
    const appSources: ApplicationAudioSource[] = applicationAudio.knownApps.map(processInfo =>
      createApplicationSource(
        processInfo,
        applicationAudio.activeCaptures.some(active => active.pid === processInfo.pid)
      )
    );
    
    return [...hardwareSources, ...appSources];
  }, [inputDevices, applicationAudio.knownApps, applicationAudio.activeCaptures]);

  // Group sources for display
  const sourceGroups = useMemo(() => groupAudioSources(audioSources), [audioSources]);

  // Create select data with groups
  const selectData = useMemo(() => {
    const data: any[] = [];
    
    sourceGroups.forEach(group => {
      // Add group header
      data.push({
        group: group.label,
        items: group.sources.map(source => ({
          value: source.id,
          label: source.displayName,
          disabled: source.type === 'application' && !applicationAudio.permissionsGranted,
        })),
      });
    });
    
    return data;
  }, [sourceGroups, applicationAudio.permissionsGranted]);

  const handleDeviceChange = useCallback((value: string | null) => {
    if (value?.startsWith('app-')) {
      // This is an application source
      const pid = parseInt(value.replace('app-', ''));
      console.log(`🎵 Selected application source PID: ${pid}`);
      
      if (!applicationAudio.permissionsGranted) {
        console.warn('⚠️ Permissions not granted - requesting...');
        applicationAudio.actions.requestPermissions();
        return;
      }
      
      // TODO: For now, just log. Later we'll integrate with mixer channels.
      const appSource = audioSources.find(s => s.id === value) as ApplicationAudioSource;
      if (appSource) {
        console.log(`🎛️ Would create mixer input for: ${appSource.name}`);
        // applicationAudio.actions.createMixerInput(pid);
      }
    }
    
    onInputDeviceChange(value);
  }, [audioSources, applicationAudio.permissionsGranted, applicationAudio.actions, onInputDeviceChange]);

  const handleRefresh = useCallback(() => {
    onRefreshDevices();
    applicationAudio.actions.refreshApplications();
  }, [onRefreshDevices, applicationAudio.actions]);

  const handleRequestPermissions = useCallback(() => {
    applicationAudio.actions.requestPermissions();
  }, [applicationAudio.actions]);

  // Debug logging
  useEffect(() => {
    if (sourceGroups.length > 0) {
      console.log('🎛️ Enhanced device selector updated:', {
        groups: sourceGroups.length,
        totalSources: audioSources.length,
        hardwareDevices: inputDevices.length,
        knownApps: applicationAudio.knownApps.length,
        permissionsGranted: applicationAudio.permissionsGranted,
      });
    }
  }, [sourceGroups.length, audioSources.length, inputDevices.length, applicationAudio.knownApps.length, applicationAudio.permissionsGranted]);

  return (
    <div>
      {/* Main device selector */}
      <Group>
        <Select
          placeholder="Select input source..."
          data={selectData}
          value={selectedDeviceId}
          onChange={handleDeviceChange}
          className={classes.selectFlex}
          size="xs"
          disabled={disabled || applicationAudio.isLoading}
          rightSection={applicationAudio.isLoading ? <Loader size={16} /> : undefined}
          clearable
        />
        <ActionIcon 
          variant="light" 
          onClick={handleRefresh} 
          title="Refresh devices and applications" 
          size="sm"
          disabled={disabled || applicationAudio.isLoading}
        >
          <IconRefresh size={16} />
        </ActionIcon>
      </Group>

      {/* Status information */}
      <div className={classes.statusText}>
        {audioSources.length > 0 && (
          <Text size="xs" c="dimmed" mt={4}>
            <IconDevices size={12} style={{ marginRight: 4, verticalAlign: 'middle' }} />
            {inputDevices.length} hardware device{inputDevices.length !== 1 ? 's' : ''}
            {applicationAudio.knownApps.length > 0 && (
              <>
                {' • '}
                <IconApps size={12} style={{ marginRight: 4, verticalAlign: 'middle' }} />
                {applicationAudio.knownApps.length} audio app{applicationAudio.knownApps.length !== 1 ? 's' : ''}
              </>
            )}
          </Text>
        )}
      </div>

      {/* Permission request for application audio */}
      {applicationAudio.knownApps.length > 0 && !applicationAudio.permissionsGranted && (
        <Alert
          icon={<IconAlertCircle size={16} />}
          title="Audio Capture Permission Required"
          color="blue"
          variant="light"
          className={classes.permissionAlert}
        >
          <Text size="sm" mb="xs">
            To use audio applications as input sources (Spotify, iTunes, etc.), microphone permission is required.
          </Text>
          <Button
            size="xs"
            onClick={handleRequestPermissions}
            loading={applicationAudio.isLoading}
          >
            Grant Permission
          </Button>
        </Alert>
      )}

      {/* Error display */}
      {applicationAudio.error && (
        <Alert
          icon={<IconAlertCircle size={16} />}
          title="Application Audio Error"
          color="red"
          variant="light"
          className={classes.permissionAlert}
          onClose={applicationAudio.actions.clearError}
        >
          <Text size="sm">{applicationAudio.error}</Text>
        </Alert>
      )}

      {/* Active captures info */}
      {applicationAudio.activeCaptures.length > 0 && (
        <Text size="xs" c="blue" mt={4}>
          ⚡ {applicationAudio.activeCaptures.length} active capture{applicationAudio.activeCaptures.length !== 1 ? 's' : ''}:
          {applicationAudio.activeCaptures.map(app => ` ${app.name}`).join(',')}
        </Text>
      )}
    </div>
  );
});