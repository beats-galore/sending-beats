import { usePatchLayoutStore } from '../../../stores/patch-layout-store';
import { pinOf } from './patch-pins';
import type { PatchRects } from './patch-rects';
import { PinSeam } from './PinSeam';

type PinSeamsProps = {
  rects: PatchRects;
};

/** Every pin currently holding two nodes together, each with its release. */
export const PinSeams = ({ rects }: PinSeamsProps) => {
  const placements = usePatchLayoutStore((state) => state.placements);

  return (
    <>
      {rects.keys.map((key) => {
        const pin = pinOf(placements[key], key);
        // A pin whose anchor has left the canvas is not holding anything, and
        // the node is already being drawn as though it were never pinned.
        if (!pin || !(pin.anchor in rects.byKey)) {
          return null;
        }

        return <PinSeam key={key} targetKey={key} pin={pin} rect={rects.byKey[key]} />;
      })}
    </>
  );
};
