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

  /**
   * Height of the two-channel meter pair as a node draws it when it has been
   * shrunk to nothing but its levels. Thin enough that a stack of shrunk nodes
   * reads as a row of meters rather than a row of cards.
   */
  compactMeterHeight: 4,

  /** Left column — one node per mixer channel. */
  source: {
    x: 36,
    top: 24,
    width: 308,
    widthExpanded: 460,
    /** Shrunk to its levels and its mute and solo, with the name above them. */
    heightCompact: 70,
    height: 172,
    /** Focused with effects off, so the inspector is only the FX switch. */
    heightInspector: 263,
    /** Focused with the effects chain showing. */
    heightExpanded: 472,
    /** Thickness of the track progress bar on an application card. */
    trackProgressHeight: 3,
    /** Between the track's two lines of type and its progress bar. */
    trackProgressGap: 4,
    /**
     * Extra room an application card takes for its track readout: two lines of
     * type at `2xs` and `3xs` on the default 1.5 line height, then the progress
     * bar and its gap, then the gap to the row below.
     */
    trackReadoutHeight: Math.ceil(10 * 1.5 + 9 * 1.5) + 4 + 3 + 8,
    gap: 20,
    /** Distance from a node's top edge to its output port centre. */
    portOffset: 59.5,
    addNodeWidth: 308,
    addNodeHeight: 92,
  },

  /** Centre column — the master sum. */
  // Middle column — one node per bus. There is no separate master sum: a bus
  // with no output is not mixed at all, so every mix on the canvas is a bus.
  bus: {
    x: 540,
    top: 60,
    width: 360,
    /** Shrunk to nothing but its levels, with the name above them. */
    heightCompact: 72,
    /** Shut: the header, a pair of meters, the FROM and TO rows and a trim. */
    height: 168,
    /** Open: adds the metering column, the large gain readout and the stats. */
    heightExpanded: 424,
    /** Vertical gap between bus nodes. */
    gap: 24,
    headerHeight: 34,
    /** Room kept for the empty-state note when there are no mixes at all. */
    emptyHeight: 96,
    /** Distance from a node's top edge to the first port centre. */
    portOffset: 59,
    /** Vertical gap between ports, capped so long member lists stay inside. */
    portSpacing: 60,
    portSpan: 300,
  },

  /** Right column — every destination the master sum feeds. */
  destination: {
    x: 1060,
    width: 344,
    /** Top of the column. Every card below flows from the one above it. */
    top: 60,
    /** Vertical gap between the cast, tape and output groups. */
    gap: 24,
    /** Shrunk to where it is sending and at what rate. */
    castHeightCompact: 72,
    castHeight: 180,
    /** Focused, with the transmitter's settings showing. */
    castHeightExpanded: 453,
    /** Shrunk to whether it is rolling and how long for. */
    tapeHeightCompact: 78,
    tapeHeight: 140,
    /** Focused, with the take's output settings showing. */
    tapeHeightExpanded: 385,
    /**
     * Distance from each card's top edge to its input port centre.
     *
     * The cable is drawn by the canvas and the dot by the card, so both have to
     * read the same number or the wire lands beside the socket.
     */
    castPortOffset: 83,
    tapePortOffset: 63,
    outputPortOffset: 24,
    /** Shrunk to the tiles saying which mixes feed it. */
    outputHeightCompact: 72,
    /**
     * Tall enough for the device row, the gain row and the row of source tiles
     * beneath them — the same three rows a shut source card carries.
     */
    outputHeight: 140,
    /** Between hardware outputs, which sit closer together than the groups above. */
    outputGap: 12,
    /** Gap between the last hardware output and the first extra destination. */
    extraOffset: 72,
    extraStep: 84,
    extraHeight: 60,
    pickerHeight: 300,
    addHeight: 56,
  },

  /** Routing tiles — the chips saying where a signal goes. */
  tile: {
    /**
     * Ceiling on a tile's width, past which the name truncates.
     *
     * A tile carries the name so the routing can be read without counting
     * numbers back to a card, but a long device name would push the row wider
     * than the node holding it, so the name gives way rather than the layout.
     */
    maxWidth: 104,
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
