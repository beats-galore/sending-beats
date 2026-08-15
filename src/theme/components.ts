import {
  Button,
  Divider,
  Input,
  InputWrapper,
  NativeSelect,
  Paper,
  Popover,
  ScrollArea,
  Switch,
  Text,
  Tooltip,
} from '@mantine/core';

import { layout } from './layout';
import { border, color } from './tokens';

// Defaults and style objects for the stock Mantine components the studio uses.
// Anything expressed here does not need repeating at a call site.

export const studioComponents = {
  Paper: Paper.extend({
    defaultProps: { radius: '2xl' },
    styles: {
      root: {
        backgroundColor: color.panel,
        border: border(),
      },
    },
  }),

  Text: Text.extend({
    defaultProps: { size: 'sm' },
  }),

  Divider: Divider.extend({
    defaultProps: { color: color.line },
  }),

  // Inputs read as inset wells cut into the panel they sit on. The border is
  // left to Mantine's own variables so the accent focus ring keeps working.
  // The well is skipped for `variant="unstyled"`. Theme styles beat the
  // variant's own CSS variables, so painting every input unconditionally gave
  // the chromeless selects on the patchbay a visible box and a grey hover —
  // they read as focused when they were not.
  Input: Input.extend({
    defaultProps: { size: 'sm', radius: 'sm' },
    styles: (_theme, props: { variant?: string }) => ({
      input: {
        backgroundColor: props.variant === 'unstyled' ? 'transparent' : color.bg,
        color: color.text,
      },
    }),
  }),

  // Field labels are tracked-out capitals sitting just above their control, so
  // `label` on any input renders the design's field pattern with no extra markup.
  InputWrapper: InputWrapper.extend({
    styles: {
      label: {
        fontSize: 'var(--mantine-font-size-2xs)',
        letterSpacing: layout.tracking.wider,
        color: color.textDim,
        marginBottom: 'var(--mantine-spacing-xs)',
      },
    },
  }),

  NativeSelect: NativeSelect.extend({
    defaultProps: { size: 'sm', radius: 'sm' },
    styles: {
      input: { cursor: 'pointer' },
      section: { color: color.textFaint },
    },
  }),

  Popover: Popover.extend({
    defaultProps: { radius: 'xl', shadow: 'md', withinPortal: true },
    styles: {
      dropdown: {
        backgroundColor: color.panel,
        border: border('acc'),
        padding: 0,
        overflow: 'hidden',
      },
    },
  }),

  Tooltip: Tooltip.extend({
    defaultProps: { radius: 'sm', withinPortal: true },
    styles: {
      tooltip: {
        backgroundColor: color.panelHi,
        border: border(),
        color: color.text,
        fontSize: 'var(--mantine-font-size-2xs)',
      },
    },
  }),

  Switch: Switch.extend({
    defaultProps: { size: 'sm', color: 'ice' },
    styles: {
      root: { '--switch-thumb-bg': color.bg },
      label: { color: color.textDim, fontSize: 'var(--mantine-font-size-sm)' },
    },
  }),

  Button: Button.extend({
    defaultProps: { size: 'xs', radius: 'sm', fw: 600 },
    styles: {
      label: { letterSpacing: layout.tracking.wide },
    },
  }),

  ScrollArea: ScrollArea.extend({
    defaultProps: { scrollbarSize: 6, type: 'hover' },
    styles: {
      thumb: { backgroundColor: color.lineStrong },
    },
  }),
};
