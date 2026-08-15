// Semantic colour tokens.
//
// Every value is a reference to a Mantine CSS variable generated from the
// tuples in `./palette`. Components import from here rather than naming shades
// directly, so the palette stays swappable and no literal colour ever reaches a
// component.

const mantineColor = (name: string, shade: number): string =>
  `var(--mantine-color-${name}-${shade})`;

export const color = {
  // Surfaces
  bg: mantineColor('surface', 9),
  bgRaised: mantineColor('surface', 8),
  panel: mantineColor('surface', 7),
  panelNav: mantineColor('surface', 6),
  panelHi: mantineColor('surface', 5),
  canvasDot: mantineColor('surface', 4),

  // Borders and inert states
  dead: mantineColor('surface', 3),
  line: mantineColor('surface', 2),
  dash: mantineColor('surface', 1),
  lineStrong: mantineColor('surface', 0),

  // Text
  text: mantineColor('carbon', 0),
  textDim: mantineColor('carbon', 2),
  textMuted: mantineColor('carbon', 4),
  textFaint: mantineColor('carbon', 6),
  textFaintest: mantineColor('carbon', 8),

  // Accent
  acc: mantineColor('ice', 4),
  accDim: mantineColor('ice', 9),

  // Playback
  playback: mantineColor('orchid', 2),
  playbackDim: mantineColor('orchid', 6),

  // Warning
  warn: mantineColor('amber', 4),
  warnBorder: mantineColor('amber', 9),

  // Hot
  hot: mantineColor('rose', 4),
  hotText: mantineColor('rose', 2),
  hotBorder: mantineColor('rose', 8),
  hotBg: mantineColor('rose', 9),
} as const;

export type ColorToken = keyof typeof color;

/** A hairline border in the given token, for `style` objects. */
export const border = (token: ColorToken = 'line'): string => `1px solid ${color[token]}`;

/** The dashed border used by "add a thing" placeholders. */
export const dashedBorder = (token: ColorToken = 'dash'): string => `1px dashed ${color[token]}`;

/** The soft bloom applied to status dots and live indicators. */
export const glow = (token: ColorToken, radius = 8): string => `0 0 ${radius}px ${color[token]}`;

/**
 * The signal gradient shared by every level meter: accent through the headroom,
 * amber approaching clip, hot at the top.
 *
 * @param deg 90 for meters that fill left to right, 0 for bottom to top.
 */
export const meterGradient = (deg: 0 | 90): string =>
  `linear-gradient(${deg}deg, ${color.acc} 0%, ${color.acc} 64%, ${color.warn} 82%, ${color.hot} 100%)`;
