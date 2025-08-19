import { invoke } from '@tauri-apps/api/core';
import React, { useState, useEffect, useRef } from 'react';

type StreamMetadata = {
  title: string;
  artist: string;
  album?: string;
  genre?: string;
};

type StreamStatus = {
  is_connected: boolean;
  is_streaming: boolean;
  current_listeners: number;
  peak_listeners: number;
  stream_duration: number;
  bitrate: number;
  error_message?: string;
};

const ListenerPlayer: React.FC = () => {
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentMetadata, _setCurrentMetadata] = useState<StreamMetadata | null>(null);
  const [streamStatus, setStreamStatus] = useState<StreamStatus | null>(null);
  const [volume, setVolume] = useState(0.8);
  const [error, setError] = useState<string>('');
  const [isLoading, setIsLoading] = useState(false);

  const audioRef = useRef<HTMLAudioElement | null>(null);
  const streamUrl = 'http://localhost:8000/live'; // Icecast stream URL

  // Update stream status periodically
  useEffect(() => {
    const updateStatus = async () => {
      try {
        const status = await invoke<StreamStatus>('get_stream_status');
        setStreamStatus(status);
      } catch (err) {
        console.error('Failed to get stream status:', err);
      }
    };

    updateStatus();
    const interval = setInterval(updateStatus, 5000); // Update every 5 seconds

    return () => clearInterval(interval);
  }, []);

  const handlePlay = async () => {
    if (!audioRef.current) return;

    try {
      setIsLoading(true);
      setError('');

      // Set the stream URL
      audioRef.current.src = streamUrl;

      // Set volume
      audioRef.current.volume = volume;

      // Start playing
      await audioRef.current.play();
      setIsPlaying(true);
    } catch (err) {
      setError(`Failed to start playback: ${err}`);
      setIsPlaying(false);
    } finally {
      setIsLoading(false);
    }
  };

  const handlePause = () => {
    if (audioRef.current) {
      audioRef.current.pause();
      setIsPlaying(false);
    }
  };

  const handleVolumeChange = (newVolume: number) => {
    setVolume(newVolume);
    if (audioRef.current) {
      audioRef.current.volume = newVolume;
    }
  };

  const handleAudioError = () => {
    setError('Failed to load audio stream. Please check your connection.');
    setIsPlaying(false);
  };

  const handleAudioLoadStart = () => {
    setIsLoading(true);
  };

  const handleAudioCanPlay = () => {
    setIsLoading(false);
  };

  const formatTime = (seconds: number): string => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;

    if (hours > 0) {
      return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
    }
    return `${minutes}:${secs.toString().padStart(2, '0')}`;
  };

  return (
    <div className="bg-surface rounded-2xl p-6 max-w-4xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-display text-brand">Sendin Beats Radio</h2>
        <div className="flex items-center gap-2">
          <div
            className={`w-3 h-3 rounded-full ${streamStatus?.is_streaming ? 'bg-accent animate-pulse' : 'bg-surface-light'}`}
          />
          <span className="text-sm text-surface-light">
            {streamStatus?.is_streaming ? 'LIVE' : 'OFFLINE'}
          </span>
        </div>
      </div>

      {error && (
        <div className="bg-accent/20 border border-accent text-accent px-4 py-2 rounded-lg mb-4">
          {error}
        </div>
      )}

      {/* Stream Status */}
      {streamStatus && (
        <div className="bg-surface-light rounded-lg p-4 mb-6">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-center">
            <div>
              <div className="text-2xl font-display text-brand">
                {streamStatus.current_listeners}
              </div>
              <div className="text-sm text-surface-light">Current Listeners</div>
            </div>
            <div>
              <div className="text-2xl font-display text-accent">{streamStatus.peak_listeners}</div>
              <div className="text-sm text-surface-light">Peak Listeners</div>
            </div>
            <div>
              <div className="text-2xl font-display text-brand">
                {formatTime(streamStatus.stream_duration)}
              </div>
              <div className="text-sm text-surface-light">Stream Duration</div>
            </div>
            <div>
              <div className="text-2xl font-display text-brand">{streamStatus.bitrate} kbps</div>
              <div className="text-sm text-surface-light">Bitrate</div>
            </div>
          </div>
        </div>
      )}

      {/* Now Playing */}
      <div className="bg-surface rounded-xl p-6 mb-6">
        <h3 className="text-lg font-display text-brand mb-4">Now Playing</h3>

        {currentMetadata ? (
          <div className="flex items-center gap-6">
            <div className="w-20 h-20 bg-brand/80 rounded-xl flex items-center justify-center text-white text-3xl font-display">
              <span role="img" aria-label="music">
                🎵
              </span>
            </div>
            <div className="flex-1">
              <div className="text-white font-display text-xl leading-tight mb-1">
                {currentMetadata.title}
              </div>
              <div className="text-accent text-sm">{currentMetadata.artist}</div>
              {currentMetadata.album && (
                <div className="text-surface-light text-xs mt-1">{currentMetadata.album}</div>
              )}
            </div>
            <div className="flex flex-col items-end">
              <span className="text-xs text-surface-light">Live</span>
              <span className="w-2 h-2 bg-accent rounded-full animate-pulse mt-1" />
            </div>
          </div>
        ) : (
          <div className="text-center text-surface-light py-8">
            <div className="text-4xl mb-4">🎵</div>
            <div className="text-lg font-display">No track information available</div>
            <div className="text-sm">Track metadata will appear here when available</div>
          </div>
        )}
      </div>

      {/* Audio Controls */}
      <div className="bg-surface rounded-xl p-6">
        <div className="flex items-center justify-center gap-4 mb-6">
          <button
            onClick={isPlaying ? handlePause : handlePlay}
            disabled={isLoading || !streamStatus?.is_streaming}
            className={`w-16 h-16 rounded-full flex items-center justify-center text-white font-medium transition-colors ${
              isLoading
                ? 'bg-surface-light cursor-not-allowed'
                : isPlaying
                  ? 'bg-accent hover:bg-accent-light'
                  : 'bg-brand hover:bg-brand-light'
            }`}
          >
            {isLoading ? (
              <div className="w-6 h-6 border-2 border-white border-t-transparent rounded-full animate-spin" />
            ) : isPlaying ? (
              <span className="text-2xl">⏸️</span>
            ) : (
              <span className="text-2xl">▶️</span>
            )}
          </button>
        </div>

        {/* Volume Control */}
        <div className="flex items-center gap-4">
          <span className="text-sm text-surface-light w-16">Volume</span>
          <div className="flex-1 bg-surface-light rounded-full h-2">
            <div
              className="bg-brand h-2 rounded-full transition-all"
              style={{ width: `${volume * 100}%` }}
            />
          </div>
          <input
            type="range"
            min="0"
            max="1"
            step="0.01"
            value={volume}
            onChange={(e) => handleVolumeChange(parseFloat(e.target.value))}
            className="w-20"
          />
          <span className="text-sm text-surface-light w-12">{Math.round(volume * 100)}%</span>
        </div>
      </div>

      {/* Stream Info */}
      <div className="mt-6 pt-6 border-t border-surface">
        <h3 className="text-lg font-display text-brand mb-4">Stream Information</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
          <div>
            <span className="text-surface-light">Stream URL:</span>
            <div className="font-mono text-brand break-all">{streamUrl}</div>
          </div>
          <div>
            <span className="text-surface-light">Status:</span>
            <div
              className={`font-medium ${streamStatus?.is_streaming ? 'text-brand' : 'text-surface-light'}`}
            >
              {streamStatus?.is_streaming ? 'Live' : 'Offline'}
            </div>
          </div>
          <div>
            <span className="text-surface-light">Quality:</span>
            <div className="font-medium">{streamStatus?.bitrate || 128} kbps</div>
          </div>
          <div>
            <span className="text-surface-light">Listeners:</span>
            <div className="font-medium">
              {streamStatus?.current_listeners || 0} / {streamStatus?.peak_listeners || 0}
            </div>
          </div>
        </div>
      </div>

      {/* Hidden audio element */}
      <audio
        ref={audioRef}
        onError={handleAudioError}
        onLoadStart={handleAudioLoadStart}
        onCanPlay={handleAudioCanPlay}
        preload="none"
      />
    </div>
  );
};

export default ListenerPlayer;
