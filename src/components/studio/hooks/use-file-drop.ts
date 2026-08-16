import { getCurrentWebview } from '@tauri-apps/api/webview';
import { useEffect, useRef, useState } from 'react';

import { asFilePath, isSupportedAudioFile } from '../../../types/file-player.types';
import type { FilePath } from '../../../types/util.types';

/**
 * A drop target for audio files dragged in from Finder.
 *
 * Files dragged onto the window are handled by the webview itself rather than by
 * the page, which is what gives us real paths — an HTML5 drop only ever hands
 * over a `File`, and the decoder needs somewhere on disk to open. The cost is
 * that the event arrives for the whole window, so where it landed is worked out
 * here by hit-testing the pointer against the element that wants it.
 */
export const useFileDrop = (onDrop: (paths: FilePath[]) => void) => {
  const ref = useRef<HTMLDivElement>(null);
  const [over, setOver] = useState(false);

  // Held in a ref so that re-rendering with a new handler does not tear the
  // window's listener down and put it back mid-drag.
  const handler = useRef(onDrop);
  handler.current = onDrop;

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;

    const covers = (position: { x: number; y: number }): boolean => {
      const box = ref.current?.getBoundingClientRect();
      if (!box) {
        return false;
      }

      // The webview reports physical pixels; the page is laid out in CSS ones.
      const scale = window.devicePixelRatio || 1;
      const x = position.x / scale;
      const y = position.y / scale;

      return x >= box.left && x <= box.right && y >= box.top && y <= box.bottom;
    };

    void getCurrentWebview()
      .onDragDropEvent(({ payload }) => {
        if (payload.type === 'leave') {
          setOver(false);
          return;
        }

        if (payload.type === 'enter' || payload.type === 'over') {
          setOver(covers(payload.position));
          return;
        }

        setOver(false);
        if (!covers(payload.position)) {
          return;
        }

        const audio = payload.paths.filter(isSupportedAudioFile).map(asFilePath);
        if (audio.length > 0) {
          handler.current(audio);
        }
      })
      .then((stop) => {
        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return { ref, over };
};
