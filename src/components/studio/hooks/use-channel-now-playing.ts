import { useNowPlayingStore } from '../../../stores/now-playing-store';
import { bundleIdFromDeviceIdentifier } from '../../../types/now-playing.types';
import type { NowPlayingTrack } from '../../../types/now-playing.types';

/**
 * What the application patched into a channel is playing.
 *
 * Null for a hardware source, for an application the backend cannot read, and
 * for one that has nothing loaded — a channel simply has no track to show.
 */
export const useChannelNowPlaying = (
  deviceIdentifier: string | null | undefined
): NowPlayingTrack | null => {
  const bundleId = deviceIdentifier ? bundleIdFromDeviceIdentifier(deviceIdentifier) : null;

  return useNowPlayingStore((state) => (bundleId ? (state.tracks[bundleId] ?? null) : null));
};
