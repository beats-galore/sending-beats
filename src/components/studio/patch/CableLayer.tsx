import { motion } from 'framer-motion';

import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';

export type Cable = {
  id: string;
  path: string;
  /**
   * The colour of whatever the wire carries.
   *
   * A run takes the colour of the thing at the far end of it — the source it
   * leaves for a send, the destination it lands on for a return — which is the
   * same rule the routing tiles follow. Tracing a patch by eye is then a matter
   * of following one colour rather than counting numbers back to a card.
   */
  color: string;
  /** Signal is flowing — the wire marches. Otherwise it sits dim and still. */
  active: boolean;
};

type CableLayerProps = {
  cables: Cable[];
  width: number;
  height: number;
};

const { patch } = layout;

/** The wiring between nodes. Purely decorative to the pointer — clicks pass through. */
export const CableLayer = ({ cables, width, height }: CableLayerProps) => (
  <svg
    width={width}
    height={height}
    style={{ position: 'absolute', left: 0, top: 0, pointerEvents: 'none' }}
  >
    {cables.map((cable, index) => (
      <g key={cable.id}>
        <path
          d={cable.path}
          fill="none"
          strokeWidth={patch.cableShadowWidth}
          strokeLinecap="round"
          stroke={cable.active ? color.accDim : color.dead}
        />
        <motion.path
          d={cable.path}
          fill="none"
          strokeWidth={patch.cableWidth}
          strokeDasharray={patch.cableDashArray}
          stroke={cable.color}
          opacity={cable.active ? 1 : 0.35}
          animate={cable.active ? { strokeDashoffset: [0, -40] } : { strokeDashoffset: 0 }}
          transition={
            cable.active
              ? { duration: 1.4 + index * 0.3, repeat: Infinity, ease: 'linear' }
              : undefined
          }
        />
      </g>
    ))}
  </svg>
);
