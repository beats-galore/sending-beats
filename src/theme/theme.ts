import { createTheme, rem } from '@mantine/core';

import type { CSSVariablesResolver } from '@mantine/core';
import { studioComponents } from './components';
import { layout } from './layout';
import { amber, carbon, dark, ice, orchid, rose, surface } from './palette';
import { color } from './tokens';

const MONO = "'IBM Plex Mono', ui-monospace, SFMono-Regular, Menlo, monospace";
const CONDENSED = "'IBM Plex Sans Condensed', 'IBM Plex Sans', system-ui, sans-serif";

export const studioTheme = createTheme({
  colors: { surface, carbon, ice, amber, orchid, rose, dark },
  primaryColor: 'ice',
  primaryShade: 4,

  // The interface is monospaced throughout; condensed sans is reserved for
  // headings and the few places that need to read as a label plate.
  fontFamily: MONO,
  fontFamilyMonospace: MONO,
  headings: {
    fontFamily: CONDENSED,
    sizes: {
      h1: { fontSize: rem(17), fontWeight: '700', lineHeight: '1.2' },
      h2: { fontSize: rem(16), fontWeight: '700', lineHeight: '1.2' },
      h3: { fontSize: rem(15), fontWeight: '700', lineHeight: '1.2' },
      h4: { fontSize: rem(14), fontWeight: '600', lineHeight: '1.2' },
      h5: { fontSize: rem(13), fontWeight: '600', lineHeight: '1.2' },
      h6: { fontSize: rem(12), fontWeight: '600', lineHeight: '1.2' },
    },
  },

  // Type runs small and dense. `md` is body copy; the steps below it carry the
  // tracked-out capital labels that separate one region of a panel from another.
  fontSizes: {
    '3xs': rem(9),
    '2xs': rem(10),
    xs: rem(11),
    sm: rem(12),
    md: rem(13),
    lg: rem(14),
    xl: rem(15),
    '2xl': rem(17),
    '3xl': rem(20),
    '4xl': rem(26),
    '5xl': rem(30),
    '6xl': rem(34),
  },

  spacing: {
    '3xs': rem(2),
    '2xs': rem(4),
    xs: rem(6),
    sm: rem(8),
    md: rem(10),
    lg: rem(12),
    xl: rem(14),
    '2xl': rem(16),
    '3xl': rem(18),
    '4xl': rem(20),
    '5xl': rem(24),
  },

  radius: {
    '2xs': rem(1),
    xs: rem(2),
    sm: rem(3),
    md: rem(4),
    lg: rem(5),
    xl: rem(6),
    '2xl': rem(8),
  },

  defaultRadius: 'sm',
  lineHeights: { xs: '1.4', sm: '1.5', md: '1.5', lg: '1.6', xl: '1.7' },
  shadows: {
    xs: '0 2px 6px rgba(0, 0, 0, 0.6)',
    sm: '0 18px 44px rgba(0, 0, 0, 0.5)',
    md: '0 18px 44px rgba(0, 0, 0, 0.55)',
    lg: '0 24px 60px rgba(0, 0, 0, 0.55)',
    xl: '0 24px 60px rgba(0, 0, 0, 0.65)',
  },

  other: { layout },
  components: studioComponents,
});

// Pin the semantic variables that stock Mantine components read, so surfaces,
// borders and placeholders follow the palette everywhere rather than only in the
// components the studio styles by hand.
export const studioCssVariablesResolver: CSSVariablesResolver = () => ({
  variables: {},
  light: {},
  dark: {
    '--mantine-color-body': color.bg,
    '--mantine-color-text': color.text,
    '--mantine-color-dimmed': color.textDim,
    '--mantine-color-default': color.panel,
    '--mantine-color-default-hover': color.panelHi,
    '--mantine-color-default-border': color.line,
    '--mantine-color-default-color': color.text,
    '--mantine-color-placeholder': color.textFaint,
    '--mantine-color-anchor': color.acc,
  },
});
