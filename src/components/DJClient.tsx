import React, { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface AudioDevice {
  deviceId: string;
  label: string;
}

interface StreamSettings {
  bitrate: number;
  sampleRate: number;
  channels: number;
}

interface StreamConfig {
  icecast_url: string;
  mount_point: string;
  username: string;
  password: string;
  bitrate: number;
  sample_rate: number;
  channels: number;
}

interface StreamStatus {
  is_connected: boolean;
  is_streaming: boolean;
  current_listeners: number;
  peak_listeners: number;
  stream_duration: number;
  bitrate: number;
  error_message?: string;
}

interface StreamMetadata {
  title: string;
  artist: string;
  album?: string;
  genre?: string;
}

const DJClient: React.FC = () => {
  const [isConnected, setIsConnected] = useState(false);
  const [isStreaming, setIsStreaming] = useState(false);
  const [selectedDevice, setSelectedDevice] = useState<string>("");
  const [audioDevices, setAudioDevices] = useState<AudioDevice[]>([]);
  const [streamSettings, setStreamSettings] = useState<StreamSettings>({
    bitrate: 128,
    sampleRate: 44100,
    channels: 2
  });
  const [streamConfig, setStreamConfig] = useState<StreamConfig>({
    icecast_url: "http://localhost:8000",
    mount_point: "live",
    username: "source",
    password: "hackme",
    bitrate: 128,
    sample_rate: 44100,
    channels: 2
  });
  const [metadata, setMetadata] = useState({
    title: "",
    artist: "",
    album: ""
  });
  const [audioLevel, setAudioLevel] = useState(0);
  const [error, setError] = useState<string>("");
  const [streamStatus, setStreamStatus] = useState<StreamStatus | null>(null);
  
  const audioContextRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const mediaStreamRef = useRef<MediaStream | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const streamIntervalRef = useRef<number | null>(null);
  const audioProcessorRef = useRef<ScriptProcessorNode | null>(null);
  const streamSenderRef = useRef<((data: Uint8Array) => void) | null>(null);

  // Get available audio devices
  useEffect(() => {
    const getAudioDevices = async () => {
      try {
        const devices = await navigator.mediaDevices.enumerateDevices();
        
        // Get all audio input devices (microphones, system audio, etc.)
        const audioInputs = devices
          .filter(device => device.kind === "audioinput")
          .map(device => ({
            deviceId: device.deviceId,
            label: device.label || `Audio Input ${device.deviceId.slice(0, 8)}`
          }));

        // Add system audio capture option if available
        const systemAudioOption = {
          deviceId: "system-audio",
          label: "System Audio (All Sounds)"
        };

        // Combine system audio with detected devices
        const allDevices = [systemAudioOption, ...audioInputs];
        
        setAudioDevices(allDevices);
        if (allDevices.length > 0) {
          setSelectedDevice(allDevices[0].deviceId);
        }
      } catch (err) {
        setError("Failed to get audio devices");
        console.error(err);
      }
    };

    getAudioDevices();
    
    // Listen for device changes
    const handleDeviceChange = () => {
      getAudioDevices();
    };
    
    navigator.mediaDevices.addEventListener('devicechange', handleDeviceChange);
    
    return () => {
      navigator.mediaDevices.removeEventListener('devicechange', handleDeviceChange);
    };
  }, []);

  // Update stream status periodically
  useEffect(() => {
    if (isConnected) {
      const updateStatus = async () => {
        try {
          const status = await invoke<StreamStatus>("get_stream_status");
          setStreamStatus(status);
        } catch (err) {
          console.error("Failed to get stream status:", err);
        }
      };

      updateStatus();
      streamIntervalRef.current = window.setInterval(updateStatus, 5000); // Update every 5 seconds

      return () => {
        if (streamIntervalRef.current) {
          window.clearInterval(streamIntervalRef.current);
        }
      };
    }
  }, [isConnected]);

  // Audio level monitoring
  const updateAudioLevel = () => {
    if (analyserRef.current) {
      const dataArray = new Uint8Array(analyserRef.current.frequencyBinCount);
      analyserRef.current.getByteFrequencyData(dataArray);
      
      const average = dataArray.reduce((sum, value) => sum + value, 0) / dataArray.length;
      setAudioLevel(average);
      
      animationFrameRef.current = requestAnimationFrame(updateAudioLevel);
    }
  };

  const startAudioMonitoring = async () => {
    try {
      if (!selectedDevice) return;

      let stream: MediaStream;

      if (selectedDevice === "system-audio") {
        // For system audio capture, we need to use a different approach
        // This will capture all system audio (requires user permission)
        stream = await navigator.mediaDevices.getUserMedia({
          audio: {
            sampleRate: streamSettings.sampleRate,
            channelCount: streamSettings.channels,
            // Try to capture system audio
            echoCancellation: false,
            noiseSuppression: false,
            autoGainControl: false
          }
        });
      } else {
        // Regular microphone/audio input device
        stream = await navigator.mediaDevices.getUserMedia({
          audio: {
            deviceId: selectedDevice,
            sampleRate: streamSettings.sampleRate,
            channelCount: streamSettings.channels
          }
        });
      }

      mediaStreamRef.current = stream;
      audioContextRef.current = new AudioContext();
      analyserRef.current = audioContextRef.current.createAnalyser();
      
      const source = audioContextRef.current.createMediaStreamSource(stream);
      source.connect(analyserRef.current);
      
      // Create audio processor for capturing raw audio data
      const processor = audioContextRef.current.createScriptProcessor(4096, 1, 1);
      processor.onaudioprocess = (event) => {
        const inputBuffer = event.inputBuffer;
        const inputData = inputBuffer.getChannelData(0);
        
        // Convert float32 to int16 for streaming
        const int16Data = new Int16Array(inputData.length);
        for (let i = 0; i < inputData.length; i++) {
          int16Data[i] = Math.max(-32768, Math.min(32767, inputData[i] * 32768));
        }
        
        // Send audio data to backend for encoding and streaming
        if (streamSenderRef.current && isStreaming) {
          const audioBytes = new Uint8Array(int16Data.buffer);
          streamSenderRef.current(audioBytes);
        }
      };
      
      source.connect(processor);
      processor.connect(audioContextRef.current.destination);
      audioProcessorRef.current = processor;
      
      updateAudioLevel();
    } catch (err) {
      setError("Failed to start audio monitoring");
      console.error(err);
    }
  };

  const stopAudioMonitoring = () => {
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current);
    }
    if (mediaStreamRef.current) {
      mediaStreamRef.current.getTracks().forEach(track => track.stop());
    }
    if (audioProcessorRef.current) {
      audioProcessorRef.current.disconnect();
    }
    if (audioContextRef.current) {
      audioContextRef.current.close();
    }
    setAudioLevel(0);
    streamSenderRef.current = null;
  };

  const connectToStream = async () => {
    try {
      setError("");
      
      // Update stream config with current settings
      const config: StreamConfig = {
        ...streamConfig,
        bitrate: streamSettings.bitrate,
        sample_rate: streamSettings.sampleRate,
        channels: streamSettings.channels
      };

      const status = await invoke<StreamStatus>("connect_to_stream", { config });
      setStreamStatus(status);
      setIsConnected(status.is_connected);
      
      if (status.is_connected) {
        await startAudioMonitoring();
      } else if (status.error_message) {
        setError(status.error_message);
      }
    } catch (err) {
      setError(`Failed to connect to stream: ${err}`);
      setIsConnected(false);
    }
  };

  const disconnectFromStream = async () => {
    try {
      await invoke("disconnect_from_stream");
      stopAudioMonitoring();
      setIsConnected(false);
      setIsStreaming(false);
      setStreamStatus(null);
      setError("");
    } catch (err) {
      setError(`Failed to disconnect: ${err}`);
    }
  };

  const startStreaming = async () => {
    if (!isConnected) return;
    
    try {
      setIsStreaming(true);
      
      // Set up audio data sender
      streamSenderRef.current = async (audioData: Uint8Array) => {
        try {
          await invoke("start_streaming", { audioData: Array.from(audioData) });
        } catch (err) {
          console.error("Failed to send audio data:", err);
          setIsStreaming(false);
        }
      };
      
    } catch (err) {
      setError(`Failed to start streaming: ${err}`);
      setIsStreaming(false);
    }
  };

  const stopStreaming = async () => {
    try {
      await invoke("stop_streaming");
      setIsStreaming(false);
      streamSenderRef.current = null;
    } catch (err) {
      setError(`Failed to stop streaming: ${err}`);
    }
  };

  const updateMetadata = async () => {
    if (!metadata.title || !metadata.artist) return;
    
    try {
      const streamMetadata: StreamMetadata = {
        title: metadata.title,
        artist: metadata.artist,
        album: metadata.album || undefined,
        genre: "Electronic"
      };
      
      await invoke("update_metadata", { metadata: streamMetadata });
      console.log("Metadata updated successfully");
    } catch (err) {
      setError(`Failed to update metadata: ${err}`);
    }
  };

  return (
    <div className="bg-surface rounded-2xl p-6 max-w-4xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-display text-brand">DJ Streaming Client</h2>
        <div className="flex items-center gap-2">
          <div className={`w-3 h-3 rounded-full ${isConnected ? 'bg-brand animate-pulse' : 'bg-surface-light'}`}></div>
          <span className="text-sm text-surface-light">
            {isConnected ? 'Connected' : 'Disconnected'}
          </span>
        </div>
      </div>

      {error && (
        <div className="bg-accent/20 border border-accent text-accent px-4 py-2 rounded-lg mb-4">
          {error}
        </div>
      )}

      {streamStatus && (
        <div className="bg-surface-light rounded-lg p-4 mb-4">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-center">
            <div>
              <div className="text-2xl font-display text-brand">{streamStatus.current_listeners}</div>
              <div className="text-sm text-surface-light">Current Listeners</div>
            </div>
            <div>
              <div className="text-2xl font-display text-accent">{streamStatus.peak_listeners}</div>
              <div className="text-sm text-surface-light">Peak Listeners</div>
            </div>
            <div>
              <div className="text-2xl font-display text-brand">{streamStatus.stream_duration}s</div>
              <div className="text-sm text-surface-light">Stream Duration</div>
            </div>
            <div>
              <div className="text-2xl font-display text-brand">{streamStatus.bitrate} kbps</div>
              <div className="text-sm text-surface-light">Bitrate</div>
            </div>
          </div>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Stream Configuration */}
        <div className="space-y-4">
          <h3 className="text-lg font-display text-brand">Stream Configuration</h3>
          
          <div>
            <label className="block text-sm font-medium text-surface-light mb-2">
              Icecast URL
            </label>
            <input
              type="text"
              value={streamConfig.icecast_url}
              onChange={(e) => setStreamConfig(prev => ({ ...prev, icecast_url: e.target.value }))}
              className="w-full bg-surface-light border border-surface rounded-lg px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-brand"
              placeholder="http://localhost:8000"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-surface-light mb-2">
              Mount Point
            </label>
            <input
              type="text"
              value={streamConfig.mount_point}
              onChange={(e) => setStreamConfig(prev => ({ ...prev, mount_point: e.target.value }))}
              className="w-full bg-surface-light border border-surface rounded-lg px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-brand"
              placeholder="live"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-surface-light mb-2">
                Username
              </label>
              <input
                type="text"
                value={streamConfig.username}
                onChange={(e) => setStreamConfig(prev => ({ ...prev, username: e.target.value }))}
                className="w-full bg-surface-light border border-surface rounded-lg px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-brand"
                placeholder="source"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-surface-light mb-2">
                Password
              </label>
              <input
                type="password"
                value={streamConfig.password}
                onChange={(e) => setStreamConfig(prev => ({ ...prev, password: e.target.value }))}
                className="w-full bg-surface-light border border-surface rounded-lg px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-brand"
                placeholder="hackme"
              />
            </div>
          </div>

          <div>
            <label className="block text-sm font-medium text-surface-light mb-2">
              Bitrate (kbps)
            </label>
            <select
              value={streamSettings.bitrate}
              onChange={(e) => setStreamSettings(prev => ({ ...prev, bitrate: Number(e.target.value) }))}
              className="w-full bg-surface-light border border-surface rounded-lg px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-brand"
            >
              <option value={64}>64 kbps</option>
              <option value={128}>128 kbps</option>
              <option value={192}>192 kbps</option>
              <option value={320}>320 kbps</option>
            </select>
          </div>

          <div className="flex gap-2">
            {!isConnected ? (
              <button
                onClick={connectToStream}
                className="flex-1 bg-brand hover:bg-brand-light text-white font-medium py-2 px-4 rounded-lg transition-colors"
              >
                Connect
              </button>
            ) : (
              <button
                onClick={disconnectFromStream}
                className="flex-1 bg-accent hover:bg-accent-light text-white font-medium py-2 px-4 rounded-lg transition-colors"
              >
                Disconnect
              </button>
            )}
          </div>
        </div>

        {/* Audio Controls */}
        <div className="space-y-4">
          <h3 className="text-lg font-display text-brand">Audio Controls</h3>
          
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="block text-sm font-medium text-surface-light">
                Audio Input Device
              </label>
              <button
                onClick={() => {
                  navigator.mediaDevices.enumerateDevices().then(devices => {
                    const audioInputs = devices
                      .filter(device => device.kind === "audioinput")
                      .map(device => ({
                        deviceId: device.deviceId,
                        label: device.label || `Audio Input ${device.deviceId.slice(0, 8)}`
                      }));
                    const systemAudioOption = {
                      deviceId: "system-audio",
                      label: "System Audio (All Sounds)"
                    };
                    const allDevices = [systemAudioOption, ...audioInputs];
                    setAudioDevices(allDevices);
                  });
                }}
                className="text-xs text-brand hover:text-brand-light transition-colors"
              >
                Refresh
              </button>
            </div>
            <select
              value={selectedDevice}
              onChange={(e) => setSelectedDevice(e.target.value)}
              className="w-full bg-surface-light border border-surface rounded-lg px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-brand"
            >
              {audioDevices.map(device => (
                <option key={device.deviceId} value={device.deviceId}>
                  {device.label}
                </option>
              ))}
            </select>
            {selectedDevice === "system-audio" && (
              <p className="text-xs text-surface-light mt-1">
                Note: System audio capture may require additional permissions and may not work in all browsers.
              </p>
            )}
          </div>

          <div>
            <label className="block text-sm font-medium text-surface-light mb-2">
              Audio Level
            </label>
            <div className="bg-surface-light rounded-lg p-4">
              <div className="flex items-end gap-1 h-20">
                {Array.from({ length: 20 }, (_, i) => (
                  <div
                    key={i}
                    className={`flex-1 rounded-sm transition-all duration-100 ${
                      i < (audioLevel / 255) * 20 ? 'bg-brand' : 'bg-surface'
                    }`}
                    style={{ height: `${Math.max(2, (audioLevel / 255) * 100)}%` }}
                  ></div>
                ))}
              </div>
              <div className="text-center text-sm text-surface-light mt-2">
                {Math.round(audioLevel)} dB
              </div>
            </div>
          </div>

          <div className="flex gap-2">
            {isConnected && (
              <>
                {!isStreaming ? (
                  <button
                    onClick={startStreaming}
                    className="flex-1 bg-brand hover:bg-brand-light text-white font-medium py-2 px-4 rounded-lg transition-colors"
                  >
                    Start Streaming
                  </button>
                ) : (
                  <button
                    onClick={stopStreaming}
                    className="flex-1 bg-accent hover:bg-accent-light text-white font-medium py-2 px-4 rounded-lg transition-colors"
                  >
                    Stop Streaming
                  </button>
                )}
              </>
            )}
          </div>
        </div>
      </div>

      {/* Metadata Section */}
      <div className="mt-6 pt-6 border-t border-surface">
        <h3 className="text-lg font-display text-brand mb-4">Track Metadata</h3>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div>
            <label className="block text-sm font-medium text-surface-light mb-2">
              Title
            </label>
            <input
              type="text"
              value={metadata.title}
              onChange={(e) => setMetadata(prev => ({ ...prev, title: e.target.value }))}
              className="w-full bg-surface-light border border-surface rounded-lg px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-brand"
              placeholder="Track title"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-surface-light mb-2">
              Artist
            </label>
            <input
              type="text"
              value={metadata.artist}
              onChange={(e) => setMetadata(prev => ({ ...prev, artist: e.target.value }))}
              className="w-full bg-surface-light border border-surface rounded-lg px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-brand"
              placeholder="Artist name"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-surface-light mb-2">
              Album
            </label>
            <input
              type="text"
              value={metadata.album}
              onChange={(e) => setMetadata(prev => ({ ...prev, album: e.target.value }))}
              className="w-full bg-surface-light border border-surface rounded-lg px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-brand"
              placeholder="Album name"
            />
          </div>
        </div>
        <button
          onClick={updateMetadata}
          disabled={!metadata.title || !metadata.artist}
          className="mt-4 bg-brand hover:bg-brand-light disabled:bg-surface disabled:cursor-not-allowed text-white font-medium py-2 px-4 rounded-lg transition-colors"
        >
          Update Metadata
        </button>
      </div>
    </div>
  );
};

export default DJClient; 