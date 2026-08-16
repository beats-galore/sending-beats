import { Box } from '@mantine/core';

import { layout } from '../../../theme/layout';
import { color } from '../../../theme/tokens';

type PlayButtonProps = {
  playing: boolean;
  /** The player's own colour: filled while playing, outlined while not. */
  tint: string;
  size?: number;
  disabled?: boolean;
  onToggle: () => void;
};

/** The round transport button, filled while running so it reads across the canvas. */
export const PlayButton = ({
  playing,
  tint,
  size = layout.source.playerButtonSize,
  disabled = false,
  onToggle,
}: PlayButtonProps) => {
  const glyphColor = (() => {
    if (playing) {
      return color.bg;
    }
    return disabled ? color.dead : tint;
  })();

  return (
  <Box
    onClick={(event) => {
      event.stopPropagation();
      if (!disabled) {
        onToggle();
      }
    }}
    title={playing ? 'Pause' : 'Play'}
    style={{
      width: size,
      height: size,
      flex: 'none',
      borderRadius: '50%',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      fontSize: Math.round(size * 0.34),
      lineHeight: 1,
      cursor: disabled ? 'default' : 'pointer',
      border: `1px solid ${disabled ? color.dead : tint}`,
      background: playing ? tint : 'transparent',
      color: glyphColor,
      // The glyphs are not the same optical weight, and a triangle sitting on
      // the geometric centre reads as being left of it.
      paddingLeft: playing ? 0 : Math.round(size * 0.06),
    }}
  >
    {playing ? '❙❙' : '▶'}
  </Box>
  );
};
