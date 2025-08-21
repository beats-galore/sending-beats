import { invoke } from '@tauri-apps/api/core';
import React, { useState, useEffect, useRef } from 'react';

// Type definitions for the audio system
// Debug: Testing compilation
type AudioDeviceInfo = {
  id: string;
  name: string;
  is_input: boolean;
  is_output: boolean;
  is_default: boolean;
  supported_sample_rates: number[];
  supported_channels: number[];
  host_api: string;
};

type AudioChannel = {
  id: number;
  name: string;
  input_device_id?: string;
  gain: number;
  pan: number;
  muted: boolean;
  solo: boolean;
  effects_enabled: boolean;
  peak_level: number;
  rms_level: number;

  // EQ settings
  eq_low_gain: number; // Low band gain in dB (-12 to +12)
  eq_mid_gain: number; // Mid band gain in dB (-12 to +12)
  eq_high_gain: number; // High band gain in dB (-12 to +12)

  // Compressor settings
  comp_threshold: number; // Threshold in dB (-40 to 0)
  comp_ratio: number; // Compression ratio (1.0 to 10.0)
  comp_attack: number; // Attack time in ms (0.1 to 100)
  comp_release: number; // Release time in ms (10 to 1000)
  comp_enabled: boolean;

  // Limiter settings
  limiter_threshold: number; // Limiter threshold in dB (-12 to 0)
  limiter_enabled: boolean;
};

type MixerConfig = {
  sample_rate: number;
  buffer_size: number;
  channels: AudioChannel[];
  master_gain: number;
  master_output_device_id?: string;
  monitor_output_device_id?: string;
  enable_loopback: boolean;
};

type AudioMetrics = {
  cpu_usage: number;
  buffer_underruns: number;
  buffer_overruns: number;
  latency_ms: number;
  sample_rate: number;
  active_channels: number;
};

// Professional VU Meter Component
const VUMeter: React.FC<{
  peakLevel: number;
  rmsLevel: number;
  vertical?: boolean;
  height?: number;
}> = ({ peakLevel, rmsLevel, vertical = true, height = 200 }) => {
  const dbPeak = peakLevel > 0 ? 20 * Math.log10(peakLevel) : -60;
  const dbRms = rmsLevel > 0 ? 20 * Math.log10(rmsLevel) : -60;

  // Convert dB to 0-1 range for visualization (-60dB to 0dB)
  const peakPosition = Math.max(0, Math.min(1, (dbPeak + 60) / 60));
  const rmsPosition = Math.max(0, Math.min(1, (dbRms + 60) / 60));

  // Debug logging for VU meter updates
  React.useEffect(() => {
    if (peakLevel > 0 || rmsLevel > 0) {
      console.debug(
        `📶 VUMeter update: peak=${peakLevel.toFixed(3)} (${dbPeak.toFixed(1)}dB), rms=${rmsLevel.toFixed(3)} (${dbRms.toFixed(1)}dB), positions=${peakPosition.toFixed(2)}/${rmsPosition.toFixed(2)}`
      );
    }
  }, [peakLevel, rmsLevel, dbPeak, dbRms, peakPosition, rmsPosition]);

  const segments = 30;
  // const segmentHeight = height / segments;

  return (
    <div className={`flex ${vertical ? 'flex-col-reverse' : 'flex-row'} gap-0.5`}>
      {Array.from({ length: segments }, (_, i) => {
        const segmentValue = (i + 1) / segments;
        const isLit = segmentValue <= peakPosition;
        const isRmsLit = segmentValue <= rmsPosition;

        // Color coding: Green (0-70%), Yellow (70-85%), Red (85-100%)
        let colorClass = 'bg-gray-600';
        if (isLit) {
          if (segmentValue < 0.7) {
            colorClass = 'bg-green-500';
          } else if (segmentValue < 0.85) {
            colorClass = 'bg-yellow-500';
          } else {
            colorClass = 'bg-red-500';
          }
        } else if (isRmsLit) {
          colorClass = 'bg-gray-400';
        }

        const segmentSize = height / segments;
        return (
          <div
            key={i}
            className={`${colorClass} transition-colors duration-75 ${vertical ? 'w-4' : 'h-4'}`}
            style={
              vertical
                ? { height: `${segmentSize}px` }
                : { width: `${segmentSize}px`, height: '4px' }
            }
          />
        );
      })}
    </div>
  );
};

// Channel Strip Component
const ChannelStrip: React.FC<{
  channel: AudioChannel;
  audioDevices: AudioDeviceInfo[];
  onChannelUpdate: (channel: AudioChannel) => void;
  onRefreshDevices: () => void;
  isRefreshingDevices: boolean;
}> = ({ channel, audioDevices, onChannelUpdate, onRefreshDevices, isRefreshingDevices }) => {
  const inputDevices = audioDevices.filter((device) => device.is_input);

  const updateChannel = (updates: Partial<AudioChannel>) => {
    onChannelUpdate({ ...channel, ...updates });
  };

  return (
    <div className="bg-surface-light p-4 rounded-lg border border-surface">
      {/* Channel Header - Horizontal Layout */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-4">
          <div>
            <h4 className="text-white font-medium">CH {channel.id}</h4>
            <p className="text-sm text-surface-light">{channel.name}</p>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => updateChannel({ muted: !channel.muted })}
              className={`px-2 py-1 text-xs font-medium rounded ${
                channel.muted
                  ? 'bg-red-600 text-white'
                  : 'bg-gray-600 text-gray-300 hover:bg-gray-500'
              }`}
            >
              MUTE
            </button>
            <button
              onClick={() => updateChannel({ solo: !channel.solo })}
              className={`px-2 py-1 text-xs font-medium rounded ${
                channel.solo
                  ? 'bg-yellow-600 text-white'
                  : 'bg-gray-600 text-gray-300 hover:bg-gray-500'
              }`}
            >
              SOLO
            </button>
          </div>
        </div>
        <div className="text-xs text-surface-light text-center">
          <div className="mb-1">VU</div>
          <VUMeter
            peakLevel={channel.peak_level}
            rmsLevel={channel.rms_level}
            height={40}
            vertical
          />
          <div className="mt-1 text-xs">
            <div>P: {channel.peak_level.toFixed(2)}</div>
          </div>
        </div>
      </div>

      {/* Horizontal Control Layout */}
      <div className="grid grid-cols-6 gap-4 mb-3">
        {/* Input Device Selection */}
        <div className="col-span-2">
          <div className="flex items-center justify-between mb-1">
            <label className="block text-xs text-surface-light">Input Device</label>
            <button
              onClick={onRefreshDevices}
              disabled={isRefreshingDevices}
              className="p-1 text-xs bg-surface border border-surface-light hover:bg-surface-light text-surface-light hover:text-white rounded disabled:opacity-50 transition-colors"
              title="Refresh audio devices"
            >
              <span className={isRefreshingDevices ? 'animate-spin' : ''}>
                {isRefreshingDevices ? '⟳' : '↻'}
              </span>
            </button>
          </div>
          <select
            value={channel.input_device_id || ''}
            onChange={(e) => updateChannel({ input_device_id: e.target.value || undefined })}
            className="w-full bg-surface text-white text-xs p-2 rounded border border-surface-light"
          >
            <option value="">No Input</option>
            {inputDevices.map((device) => (
              <option key={device.id} value={device.id}>
                {device.name}
              </option>
            ))}
          </select>
        </div>

        {/* Gain Control */}
        <div>
          <label className="block text-xs text-surface-light mb-1">
            Gain: {(20 * Math.log10(channel.gain)).toFixed(1)} dB
          </label>
          <input
            type="range"
            min="0.1"
            max="2.0"
            step="0.05"
            value={channel.gain}
            onChange={(e) => updateChannel({ gain: parseFloat(e.target.value) })}
            className="w-full"
          />
        </div>

        {/* Pan Control */}
        <div>
          <label className="block text-xs text-surface-light mb-1">
            Pan:{' '}
            {channel.pan > 0
              ? `R${(channel.pan * 100).toFixed(0)}`
              : channel.pan < 0
                ? `L${Math.abs(channel.pan * 100).toFixed(0)}`
                : 'Center'}
          </label>
          <input
            type="range"
            min="-1"
            max="1"
            step="0.05"
            value={channel.pan}
            onChange={(e) => updateChannel({ pan: parseFloat(e.target.value) })}
            className="w-full"
          />
        </div>

        {/* EQ Controls - Condensed */}
        <div>
          <label className="block text-xs text-surface-light mb-1">EQ High</label>
          <input
            type="range"
            min="-12"
            max="12"
            step="0.5"
            value={channel.eq_high_gain}
            onChange={(e) => updateChannel({ eq_high_gain: parseFloat(e.target.value) })}
            className="w-full"
          />
          <div className="text-xs text-center">{channel.eq_high_gain.toFixed(1)}dB</div>
        </div>

        <div>
          <label className="block text-xs text-surface-light mb-1">EQ Mid/Low</label>
          <input
            type="range"
            min="-12"
            max="12"
            step="0.5"
            value={channel.eq_mid_gain}
            onChange={(e) => updateChannel({ eq_mid_gain: parseFloat(e.target.value) })}
            className="w-full mb-1"
          />
          <input
            type="range"
            min="-12"
            max="12"
            step="0.5"
            value={channel.eq_low_gain}
            onChange={(e) => updateChannel({ eq_low_gain: parseFloat(e.target.value) })}
            className="w-full"
          />
          <div className="text-xs text-center">
            {channel.eq_mid_gain.toFixed(1)}/{channel.eq_low_gain.toFixed(1)}
          </div>
        </div>
      </div>

      {/* Effects Controls - Horizontal Layout */}
      <div className="grid grid-cols-4 gap-4">
        {/* Compressor Controls */}
        <div className="col-span-2">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs text-surface-light">Compressor</span>
            <button
              onClick={() => updateChannel({ comp_enabled: !channel.comp_enabled })}
              className={`px-2 py-1 text-xs font-medium rounded ${
                channel.comp_enabled
                  ? 'bg-green-600 text-white'
                  : 'bg-gray-600 text-gray-300 hover:bg-gray-500'
              }`}
            >
              {channel.comp_enabled ? 'ON' : 'OFF'}
            </button>
          </div>
          {channel.comp_enabled && (
            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="block text-xs text-surface-light">Threshold</label>
                <input
                  type="range"
                  min="-40"
                  max="0"
                  step="1"
                  value={channel.comp_threshold}
                  onChange={(e) => updateChannel({ comp_threshold: parseFloat(e.target.value) })}
                  className="w-full"
                />
                <div className="text-xs text-center">{channel.comp_threshold.toFixed(1)} dB</div>
              </div>
              <div>
                <label className="block text-xs text-surface-light">Ratio</label>
                <input
                  type="range"
                  min="1"
                  max="10"
                  step="0.1"
                  value={channel.comp_ratio}
                  onChange={(e) => updateChannel({ comp_ratio: parseFloat(e.target.value) })}
                  className="w-full"
                />
                <div className="text-xs text-center">{channel.comp_ratio.toFixed(1)}:1</div>
              </div>
              <div>
                <label className="block text-xs text-surface-light">Attack</label>
                <input
                  type="range"
                  min="0.1"
                  max="100"
                  step="0.1"
                  value={channel.comp_attack}
                  onChange={(e) => updateChannel({ comp_attack: parseFloat(e.target.value) })}
                  className="w-full"
                />
                <div className="text-xs text-center">{channel.comp_attack.toFixed(1)} ms</div>
              </div>
              <div>
                <label className="block text-xs text-surface-light">Release</label>
                <input
                  type="range"
                  min="10"
                  max="1000"
                  step="10"
                  value={channel.comp_release}
                  onChange={(e) => updateChannel({ comp_release: parseFloat(e.target.value) })}
                  className="w-full"
                />
                <div className="text-xs text-center">{channel.comp_release.toFixed(0)} ms</div>
              </div>
            </div>
          )}
        </div>

        {/* Limiter Controls */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs text-surface-light">Limiter</span>
            <button
              onClick={() => updateChannel({ limiter_enabled: !channel.limiter_enabled })}
              className={`px-2 py-1 text-xs font-medium rounded ${
                channel.limiter_enabled
                  ? 'bg-orange-600 text-white'
                  : 'bg-gray-600 text-gray-300 hover:bg-gray-500'
              }`}
            >
              {channel.limiter_enabled ? 'ON' : 'OFF'}
            </button>
          </div>
          {channel.limiter_enabled && (
            <div>
              <label className="block text-xs text-surface-light">Threshold</label>
              <input
                type="range"
                min="-12"
                max="0"
                step="0.1"
                value={channel.limiter_threshold}
                onChange={(e) => updateChannel({ limiter_threshold: parseFloat(e.target.value) })}
                className="w-full"
              />
              <div className="text-xs text-center">{channel.limiter_threshold.toFixed(1)} dB</div>
            </div>
          )}
        </div>

        {/* Spacer */}
        <div />
      </div>
    </div>
  );
};

// Main Virtual Mixer Component
const VirtualMixer: React.FC = () => {
  const [mixerConfig, setMixerConfig] = useState<MixerConfig | null>(null);
  const [audioDevices, setAudioDevices] = useState<AudioDeviceInfo[]>([]);
  const [metrics, setMetrics] = useState<AudioMetrics | null>(null);
  const [masterLevels, setMasterLevels] = useState<{
    leftPeak: number;
    leftRms: number;
    rightPeak: number;
    rightRms: number;
  }>({
    leftPeak: 0,
    leftRms: 0,
    rightPeak: 0,
    rightRms: 0,
  });
  const [isRunning, setIsRunning] = useState(true); // Always running after initialization
  const [nextChannelId, setNextChannelId] = useState(1);
  const [error, setError] = useState<string>('');
  const [isRefreshingDevices, setIsRefreshingDevices] = useState(false);

  const metricsIntervalRef = useRef<number | null>(null);

  // Initialize mixer
  useEffect(() => {
    const initializeMixer = async () => {
      try {
        // Get DJ-optimized config
        const djConfig = await invoke<MixerConfig>('get_dj_mixer_config');
        setMixerConfig(djConfig);
        setNextChannelId(djConfig.channels.length + 1);

        // Enumerate audio devices
        const devices = await invoke<AudioDeviceInfo[]>('enumerate_audio_devices');
        setAudioDevices(devices);

        // Create mixer with config (automatically starts in always-running mode)
        await invoke('create_mixer', { config: djConfig });
        setIsRunning(true); // Mixer is now always running after creation
      } catch (err) {
        setError(`Failed to initialize mixer: ${err}`);
      }
    };

    initializeMixer();
  }, []);

  // Start metrics polling when mixer is running
  useEffect(() => {
    if (isRunning) {
      const pollMetrics = async () => {
        try {
          console.debug('🔍 Polling VU meter data...');
          const currentMetrics = await invoke<AudioMetrics>('get_mixer_metrics');
          setMetrics(currentMetrics);

          // Get real-time channel levels for VU meters
          const channelLevels =
            await invoke<Record<number, [number, number]>>('get_channel_levels');
          console.debug('📊 Received channel levels:', channelLevels);

          // Get real-time master levels for VU meters
          const masterLevelsData =
            await invoke<[number, number, number, number]>('get_master_levels');
          console.debug('📊 Received master levels:', masterLevelsData);
          setMasterLevels({
            leftPeak: masterLevelsData[0],
            leftRms: masterLevelsData[1],
            rightPeak: masterLevelsData[2],
            rightRms: masterLevelsData[3],
          });

          // Update mixer config with new levels
          setMixerConfig((prev) => {
            if (!prev) return prev;
            return {
              ...prev,
              channels: prev.channels.map((channel) => {
                const levels = channelLevels[channel.id];
                if (levels) {
                  console.debug(
                    `🎚️ Channel ${channel.id} levels: peak=${levels[0].toFixed(3)}, rms=${levels[1].toFixed(3)}`
                  );
                  return {
                    ...channel,
                    peak_level: levels[0],
                    rms_level: levels[1],
                  };
                }
                return channel;
              }),
            };
          });
        } catch (err) {
          console.error('❌ Failed to get metrics:', err);
        }
      };

      console.debug('🚀 Starting VU meter polling...');
      pollMetrics(); // Initial call
      metricsIntervalRef.current = window.setInterval(pollMetrics, 100); // 10 FPS for smooth VU meters

      return () => {
        if (metricsIntervalRef.current) {
          console.debug('🛑 Stopping VU meter polling...');
          window.clearInterval(metricsIntervalRef.current);
        }
      };
    }
  }, [isRunning]);

  // Start/stop functions removed - mixer is now always running after initialization

  const addChannel = async () => {
    if (!mixerConfig) return;

    const newChannel: AudioChannel = {
      id: nextChannelId,
      name: `Channel ${nextChannelId}`,
      gain: 1.0,
      pan: 0.0,
      muted: false,
      solo: false,
      effects_enabled: false,
      peak_level: 0.0,
      rms_level: 0.0,

      // EQ defaults (flat response)
      eq_low_gain: 0.0,
      eq_mid_gain: 0.0,
      eq_high_gain: 0.0,

      // Compressor defaults
      comp_threshold: -12.0,
      comp_ratio: 4.0,
      comp_attack: 5.0,
      comp_release: 100.0,
      comp_enabled: false,

      // Limiter defaults
      limiter_threshold: -0.1,
      limiter_enabled: false,
    };

    try {
      await invoke('add_mixer_channel', { channel: newChannel });
      setMixerConfig((prev) =>
        prev
          ? {
              ...prev,
              channels: [...prev.channels, newChannel],
            }
          : null
      );
      setNextChannelId((prev) => prev + 1);
    } catch (err) {
      setError(`Failed to add channel: ${err}`);
    }
  };

  const handleChannelUpdate = async (updatedChannel: AudioChannel) => {
    try {
      // Get the previous channel configuration
      const previousChannel = mixerConfig?.channels.find((ch) => ch.id === updatedChannel.id);
      const previousInputDeviceId = previousChannel?.input_device_id;
      const newInputDeviceId = updatedChannel.input_device_id;

      // Update channel configuration first
      await invoke('update_mixer_channel', {
        channelId: updatedChannel.id,
        channel: updatedChannel,
      });

      // If input device was added or changed, create/update input stream
      if (newInputDeviceId && newInputDeviceId !== previousInputDeviceId) {
        console.debug(`🎤 Adding input stream for device: ${newInputDeviceId}`);
        try {
          await invoke('add_input_stream', { deviceId: newInputDeviceId });
          console.debug(`✅ Successfully added input stream for: ${newInputDeviceId}`);
        } catch (streamErr) {
          console.error(`❌ Failed to add input stream for ${newInputDeviceId}:`, streamErr);
          setError(`Failed to add input stream: ${streamErr}`);
        }
      }

      // If input device was removed, remove input stream
      if (previousInputDeviceId && !newInputDeviceId) {
        console.debug(`🗑️ Removing input stream for device: ${previousInputDeviceId}`);
        try {
          await invoke('remove_input_stream', { deviceId: previousInputDeviceId });
          console.debug(`✅ Successfully removed input stream for: ${previousInputDeviceId}`);
        } catch (streamErr) {
          console.error(
            `❌ Failed to remove input stream for ${previousInputDeviceId}:`,
            streamErr
          );
        }
      }

      // Update local state
      setMixerConfig((prev) =>
        prev
          ? {
              ...prev,
              channels: prev.channels.map((ch) =>
                ch.id === updatedChannel.id ? updatedChannel : ch
              ),
            }
          : null
      );
    } catch (err) {
      setError(`Failed to update channel: ${err}`);
    }
  };

  const refreshAudioDevices = async () => {
    setIsRefreshingDevices(true);
    try {
      const refreshedDevices = await invoke<AudioDeviceInfo[]>('refresh_audio_devices');
      setAudioDevices(refreshedDevices);
      console.debug(`🔄 Refreshed audio devices: ${refreshedDevices.length} devices found`);
    } catch (err) {
      setError(`Failed to refresh audio devices: ${err}`);
      console.error('❌ Failed to refresh audio devices:', err);
    } finally {
      setIsRefreshingDevices(false);
    }
  };

  if (!mixerConfig) {
    return (
      <div className="flex items-center justify-center h-96">
        <div className="text-center">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-brand mb-4" />
          <div className="text-surface-light">Initializing Virtual Mixer...</div>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-surface-dark p-6 rounded-2xl">
      <h2 className="text-3xl font-display text-brand mb-6">Virtual Audio Mixer</h2>
      <div className="text-white">
        <p>Mixer Config: {mixerConfig ? 'Loaded' : 'Loading...'}</p>
        <p>Audio Devices: {audioDevices.length}</p>
        <p>Is Running: {isRunning ? 'Yes' : 'No'}</p>
        <p>Error: {error || 'None'}</p>

        <div className="mt-4">
          <button
            onClick={addChannel}
            className="bg-surface border border-surface-light hover:bg-surface-light text-white px-4 py-2 rounded-lg"
          >
            + Add Channel
          </button>
        </div>

        {/* Channel Strips */}
        {mixerConfig && (
          <div className="mt-6">
            <h3 className="text-lg font-display text-brand mb-4">Channel Strips:</h3>
            <div className="space-y-4">
              {mixerConfig.channels.map((channel) => (
                <ChannelStrip
                  key={channel.id}
                  channel={channel}
                  audioDevices={audioDevices}
                  onChannelUpdate={handleChannelUpdate}
                  onRefreshDevices={refreshAudioDevices}
                  isRefreshingDevices={isRefreshingDevices}
                />
              ))}
            </div>
          </div>
        )}

        {/* Master Section */}
        {mixerConfig && (
          <div className="mt-6 bg-surface-light p-6 rounded-lg border-2 border-brand">
            <h3 className="text-lg font-display text-brand mb-4">Master Section</h3>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              {/* Master Output Device Selection */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <label className="block text-sm text-surface-light">Master Output Device</label>
                  <button
                    onClick={refreshAudioDevices}
                    disabled={isRefreshingDevices}
                    className="p-2 text-sm bg-surface border border-surface-light hover:bg-surface-light text-surface-light hover:text-white rounded disabled:opacity-50 transition-colors"
                    title="Refresh audio devices"
                  >
                    <span className={isRefreshingDevices ? 'animate-spin' : ''}>
                      {isRefreshingDevices ? '⟳' : '↻'}
                    </span>
                  </button>
                </div>
                <select
                  value={mixerConfig.master_output_device_id || ''}
                  onChange={async (e) => {
                    const deviceId = e.target.value || undefined;
                    try {
                      if (deviceId) {
                        await invoke('set_output_stream', { deviceId });
                      }
                      setMixerConfig((prev) =>
                        prev
                          ? {
                              ...prev,
                              master_output_device_id: deviceId,
                            }
                          : null
                      );
                    } catch (err) {
                      setError(`Failed to set output device: ${err}`);
                    }
                  }}
                  className="w-full bg-surface text-white p-3 rounded border border-surface-light"
                >
                  <option value="">Select Output Device</option>
                  {audioDevices
                    .filter((device) => device.is_output)
                    .map((device) => (
                      <option key={device.id} value={device.id}>
                        {device.name} ({device.host_api})
                      </option>
                    ))}
                </select>
              </div>

              {/* Master Gain Control */}
              <div>
                <label className="block text-sm text-surface-light mb-2">
                  Master Gain: {(20 * Math.log10(mixerConfig.master_gain)).toFixed(1)} dB
                </label>
                <input
                  type="range"
                  min="0.1"
                  max="1.5"
                  step="0.05"
                  value={mixerConfig.master_gain}
                  onChange={async (e) => {
                    const newGain = parseFloat(e.target.value);
                    setMixerConfig((prev) =>
                      prev
                        ? {
                            ...prev,
                            master_gain: newGain,
                          }
                        : null
                    );
                  }}
                  className="w-full"
                />
              </div>
            </div>

            {/* Master VU Meters */}
            <div className="mt-4 flex items-center justify-center gap-6">
              <div className="text-center">
                <div className="text-sm text-surface-light mb-2">Master L</div>
                <VUMeter
                  peakLevel={masterLevels.leftPeak}
                  rmsLevel={masterLevels.leftRms}
                  height={120}
                  vertical
                />
                <div className="text-xs text-surface-light mt-2">
                  <div>Peak: {masterLevels.leftPeak.toFixed(3)}</div>
                  <div>RMS: {masterLevels.leftRms.toFixed(3)}</div>
                </div>
              </div>

              <div className="text-center">
                <div className="text-sm text-surface-light mb-2">Master R</div>
                <VUMeter
                  peakLevel={masterLevels.rightPeak}
                  rmsLevel={masterLevels.rightRms}
                  height={120}
                  vertical
                />
                <div className="text-xs text-surface-light mt-2">
                  <div>Peak: {masterLevels.rightPeak.toFixed(3)}</div>
                  <div>RMS: {masterLevels.rightRms.toFixed(3)}</div>
                </div>
              </div>
            </div>

            {/* Audio Metrics */}
            {metrics && (
              <div className="mt-4 grid grid-cols-2 md:grid-cols-4 gap-4 text-xs">
                <div className="bg-surface p-2 rounded">
                  <div className="text-surface-light">CPU Usage</div>
                  <div className="text-white font-medium">{metrics.cpu_usage.toFixed(1)}%</div>
                </div>
                <div className="bg-surface p-2 rounded">
                  <div className="text-surface-light">Sample Rate</div>
                  <div className="text-white font-medium">{metrics.sample_rate} Hz</div>
                </div>
                <div className="bg-surface p-2 rounded">
                  <div className="text-surface-light">Latency</div>
                  <div className="text-white font-medium">{metrics.latency_ms.toFixed(1)} ms</div>
                </div>
                <div className="bg-surface p-2 rounded">
                  <div className="text-surface-light">Active Channels</div>
                  <div className="text-white font-medium">{metrics.active_channels}</div>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

export default VirtualMixer;
