// The Ice palette.
//
// This module is the ONLY place in the application where a literal colour value
// may appear. Every other module references the Mantine CSS variables that these
// tuples generate (`--mantine-color-<name>-<shade>`), reached through the
// semantic names in `./tokens`.
//
// Mantine tuple convention: index 0 is the lightest shade, index 9 the darkest.
import type { MantineColorsTuple } from '@mantine/core';

// Surfaces and hairlines: the strongest border (0) down to the app background (9).
export const surface: MantineColorsTuple = [
  '#263640', // strong divider
  '#1F2C34', // dashed placeholder border
  '#1B2830', // default hairline
  '#17232B', // inert / disconnected
  '#14202A', // canvas dot grid
  '#101921', // raised panel, hover
  '#0E1720', // nav well
  '#0E151A', // panel
  '#0B1116', // panel header, app chrome
  '#05080A', // app background, inset wells
];

// Foreground text: brightest (0) down to the faintest legible tone (9).
export const carbon: MantineColorsTuple = [
  '#DCE9F0', // primary text
  '#B5C8D3',
  '#93A8B4', // secondary text
  '#7E95A2',
  '#6B8290', // dimmed text
  '#5C7280',
  '#4E626E', // faint labels
  '#445660',
  '#3A4A54', // faintest text, muted state
  '#2F3D46',
];

// Accent — signal, live audio, selection.
export const ice: MantineColorsTuple = [
  '#E6FAFE',
  '#C9F4FC',
  '#9DEBF9',
  '#71E2F5',
  '#4FD8F0', // accent
  '#38C3DB',
  '#29A7BD',
  '#1E7D8E',
  '#17545F',
  '#16303A', // accent wash, cable shadow
];

// Warning — cue sends, application taps, gain reduction.
export const amber: MantineColorsTuple = [
  '#FDF6E4',
  '#FAEBC2',
  '#F7DE9A',
  '#F4D072',
  '#F0C24F', // warn
  '#D9A93B',
  '#B88B2C',
  '#8E6A21',
  '#654B18',
  '#463B18', // warn border
];

// Mantine's built-in `dark` scale, remapped onto the palette above.
//
// Stock Mantine components read specific shades of `dark` directly — inputs take
// their border from shade 4 and their background from shade 6, hover states from
// shade 5. Pointing those shades at the design's tokens makes every stock
// component inherit the palette with no per-component styling, and keeps focus
// and hover states working through Mantine's own CSS rather than inline styles.
export const dark: MantineColorsTuple = [
  carbon[0], // text
  carbon[2], // secondary text
  carbon[4], // dimmed text
  carbon[6], // faint text
  surface[2], // input and default border
  surface[5], // hover
  surface[9], // input well
  surface[7], // component surface
  surface[8], // chrome
  surface[9], // background
];

// Hot — on air, recording, destructive.
export const rose: MantineColorsTuple = [
  '#FFE9ED',
  '#FFCDD6',
  '#FF9AAB', // hot text
  '#FF7A90',
  '#FF5C77', // hot
  '#E04862',
  '#B93A4F',
  '#8A2B3B',
  '#43202B', // hot border
  '#170D11', // hot wash
];
