// Semantic colour tokens.
//
// Every value is a reference to a Mantine CSS variable generated from the
// tuples in `./palette`. Components import from here rather than naming shades
// directly, so the palette stays swappable and no literal colour ever reaches a
// component.

import { swatch } from './palette';

const mantineColor = (name: string, shade: number): string =>
  `var(--mantine-color-${name}-${shade})`;

/**
 * The identity swatches, in the order the picker offers them.
 *
 * Unlike the semantic tokens below these are not generated as Mantine shades:
 * they are read back from what the user chose, so a component needs the value
 * for an arbitrary key rather than a fixed name.
 */
export const Swatch = ['saffron', 'mint', 'blossom', 'iris', 'ember'] as const;
export type Swatch = (typeof Swatch)[number];

export const isSwatch = (value: string): value is Swatch =>
  Swatch.includes(value as Swatch);

export const swatchColor = (key: Swatch): string => swatch[key];

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
 * The signal gradient shared by every level meter: the signal's own colour
 * through the headroom, amber approaching clip, hot at the top.
 *
 * `base` is what the meter reads as while there is room to spare, so a source
 * meters in the colour it was given and a glance across the canvas says which
 * signal is which without reading a single label. Only the headroom is coloured
 * that way — the warning at the top means the same thing on every meter, and a
 * source that happened to be given amber must not look like one that is
 * clipping.
 *
 * @param deg 90 for meters that fill left to right, 0 for bottom to top.
 */
export const meterGradient = (deg: 0 | 90, base: string = color.acc): string =>
  `linear-gradient(${deg}deg, ${base} 0%, ${base} 64%, ${color.warn} 82%, ${color.hot} 100%)`;
