import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useCallback, useEffect, useRef, useState } from 'react';

import { asFilePath, isSupportedAudioFile } from '../../../types/file-player.types';
import type { FilePath } from '../../../types/util.types';

/**
 * How far the page's own coordinates sit inside the window's.
 *
 * A drop is reported against the window, which on macOS includes the title bar
 * above the page. Comparing that against an element's box — which is measured
 * from the top of the page — misses by exactly the height of that bar, and a
 * drop target only a few rows tall misses entirely.
 *
 * Measured rather than assumed: it is a title bar here and nothing at all on a
 * window without decorations.
 */
type WindowChrome = {
  scale: number;
  dx: number;
  dy: number;
};

const NO_CHROME: WindowChrome = { scale: 1, dx: 0, dy: 0 };

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

  const chrome = useRef<WindowChrome>(NO_CHROME);

  const measureChrome = useCallback(async () => {
    try {
      const win = getCurrentWindow();
      const [size, scale] = await Promise.all([win.innerSize(), win.scaleFactor()]);

      chrome.current = {
        scale,
        dx: size.width / scale - window.innerWidth,
        dy: size.height / scale - window.innerHeight,
      };
    } catch {
      // Nothing to correct by is better than refusing to accept a drop at all.
      chrome.current = { ...NO_CHROME, scale: window.devicePixelRatio || 1 };
    }
  }, []);

  useEffect(() => {
    const remeasure = () => void measureChrome();
    remeasure();

    // The bar does not change height, but the viewport does, and the offset is
    // the difference between the two.
    window.addEventListener('resize', remeasure);
    return () => window.removeEventListener('resize', remeasure);
  }, [measureChrome]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;

    const covers = (position: { x: number; y: number }): boolean => {
      const box = ref.current?.getBoundingClientRect();
      if (!box) {
        return false;
      }

      const { scale, dx, dy } = chrome.current;
      const x = position.x / scale - dx;
      const y = position.y / scale - dy;

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
