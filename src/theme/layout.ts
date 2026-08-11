// Layout metrics for the studio shell and the patchbay canvas.
//
// The patchbay positions nodes absolutely and draws bezier cables between them,
// so it needs a shared coordinate system rather than flow layout. These values
// define that system; the canvas is authored at `canvas.width` and scaled to
// whatever width it is given. Everything outside the canvas lays out fluidly.

export const layout = {
  /** Application chrome. */
  shell: {
    topBarHeight: 56,
    drawerWidth: 340,
    drawerRailWidth: 38,
  },

  /** Letter spacing steps. The design leans on tracking to separate label tiers. */
  tracking: {
    tight: '0.06em',
    label: '0.08em',
    wide: '0.1em',
    wider: '0.12em',
    caps: '0.14em',
    heading: '0.16em',
    section: '0.18em',
    widest: '0.2em',
  },

  canvas: {
    /** Logical width the patchbay is authored against, before scale-to-fit. */
    width: 1440,
    minHeight: 600,
    dotGridSize: 26,
    /** Vertical room kept below the last node for the "patch a source" target. */
    bottomPadding: 116,
  },

  /** Left column — one node per mixer channel. */
  source: {
    x: 36,
    top: 24,
    width: 308,
    widthExpanded: 460,
    height: 152,
    heightExpanded: 452,
    gap: 20,
    /** Distance from a node's top edge to its output port centre. */
    portOffset: 59.5,
    addNodeWidth: 308,
    addNodeHeight: 92,
  },

  /** Centre column — the master sum. */
  bus: {
    x: 540,
    top: 120,
    width: 360,
    height: 360,
    headerHeight: 34,
    /** Distance from the bus top edge to the first input port centre. */
    portOffset: 59,
    /** Vertical gap between ports, capped so long channel lists stay inside. */
    portSpacing: 60,
    portSpan: 300,
    /** Distance from the bus top edge to the first output port centre. */
    outPortOffset: 79,
  },

  /** Right column — every destination the master sum feeds. */
  destination: {
    x: 1060,
    width: 344,
    castTop: 60,
    castHeight: 180,
    tapeTop: 264,
    tapeHeight: 140,
    outputTop: 428,
    outputStep: 96,
    outputHeight: 84,
    /** Gap between the last hardware output and the first extra destination. */
    extraOffset: 72,
    extraStep: 84,
    extraHeight: 60,
    pickerHeight: 300,
    addHeight: 56,
  },

  /** Cable and port geometry. */
  patch: {
    portSize: 13,
    /** Half the port size — ports straddle the panel edge they belong to. */
    portInset: -7,
    /** Horizontal reach of the bezier control points. */
    cableControlReach: 60,
    cableWidth: 2,
    cableShadowWidth: 7,
    cableDashArray: '10 14',
  },
} as const;

export type StudioLayout = typeof layout;
