//! The records: what the platform keeps per node, and the environment it reports.

use std::rc::Rc;

use rustc_hash::FxHashMap;
use skene_foundation::{
  Availability,
  BoxProps,
  ClipboardAction,
  ColorScheme,
  ControlProps,
  Definitions,
  Environment,
  EventKind,
  Inset,
  MediaSignals,
  NodeId,
  NodeMap,
  OverlayProps,
  PickerOption,
  Point,
  PointerPrecision,
  PropertyKey,
  RunProps,
  Size,
  Slot,
  TextSelection,
  Timers,
  interpolate::Motion,
  web::{Head, Painter, WebProperty},
};
use skene_layout::{Frames, Restyle};
use skene_reactive::{Owner, ReadSignal, Signal, WriteSignal};

use crate::{Headless, SlotLeaf, SlotValue, TokenValue};

/// The viewport the platform reports until [`trigger_resize`](crate::trigger_resize) is
/// called.
pub(crate) const DEFAULT_VIEWPORT: Size<f32> = Size::new(1024., 768.);

/// What a record is, with everything its creation call carried.
pub(crate) enum Kind {
  /// A box, with the props it was created from.
  Box(Box<BoxProps<Headless>>),
  /// A control, with the props it was created from.
  Control(Box<ControlProps<Headless>>),
  /// An overlay, with the props it was created from.
  Overlay(Box<OverlayProps<Headless>>),
  /// A text run, with the props it was created from.
  Run(Box<RunProps<Headless>>),
  /// An anchor.
  Anchor,
  /// A scope.
  Scope,
}

/// The motion a node runs: the value each resolved property is heading for, and the
/// runner its transitions and animations play through.
#[derive(Default)]
pub(crate) struct NodeMotion {
  /// The value last resolved per property, which a change is judged against.
  pub targets: FxHashMap<PropertyKey<WebProperty>, Option<SlotValue>>,
  /// The tweens in flight, and the clock the animations count from: the node's
  /// creation.
  pub runner: Motion<WebProperty, SlotValue>,
}

/// How a frame reports a box's laid-out size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Report {
  /// The size, where it changed since the last report.
  Changed,
  /// The size, changed or not: an observer was bound since the last report.
  Due,
  /// Nothing: a test reported the size by hand, which holds against the layout.
  ByHand,
}

/// One realized node.
pub(crate) struct Record {
  /// What the node is, with the props it was created from.
  pub kind: Kind,
  /// A run's text, as last set.
  pub text: String,
  /// A control's value, as last set.
  pub value: String,
  /// The options a platform-drawn picker offers.
  pub options: Vec<PickerOption>,
  /// A text entry's selection, clamped into its value.
  pub selection: TextSelection,
  /// Whether a native toggle is checked, as last set.
  pub checked: bool,
  /// Whether an overlay is open.
  pub open: bool,
  /// The point an overlay is placed at, where it is pinned to one.
  pub position: Option<Point<f32>>,
  /// The offsets the box was last scrolled to.
  pub scroll: Point<f32>,
  /// The node's slot values, as last written.
  pub slots: FxHashMap<Slot, SlotLeaf>,
  /// A scope's token definitions, as last written.
  pub definitions: Option<Definitions<TokenValue>>,
  /// The events the node listens to, in `EventKind` order.
  pub listened: Vec<EventKind>,
  /// The reports of the box's size.
  pub size_observers: Vec<Rc<dyn Fn(Size<f32>)>>,
  /// The size last reported for the box.
  pub size: Option<Size<f32>>,
  /// How a frame reports the box's laid-out size.
  pub report: Report,
  /// The painter a painted box draws with.
  pub painter: Option<Rc<Painter>>,
  /// The motion the node runs.
  pub motion: NodeMotion,
}

impl Kind {
  /// The box props of a box or a control.
  pub(crate) fn box_props(&self) -> Option<&BoxProps<Headless>> {
    match self {
      Self::Box(props) => Some(props),
      Self::Control(props) => Some(&props.box_props),
      Self::Overlay(props) => Some(&props.box_props),
      Self::Run(_) | Self::Anchor | Self::Scope => None,
    }
  }
}

impl Record {
  pub(crate) fn new(kind: Kind) -> Self {
    Self {
      kind,
      text: String::new(),
      value: String::new(),
      options: Vec::new(),
      selection: TextSelection::default(),
      checked: false,
      open: false,
      position: None,
      scroll: Point::new(0., 0.),
      slots: FxHashMap::default(),
      definitions: None,
      listened: Vec::new(),
      size_observers: Vec::new(),
      size: None,
      report: Report::Changed,
      painter: None,
      motion: NodeMotion::default(),
    }
  }
}

/// The environment as signals, with the write halves the seams drive.
pub(crate) struct MediaState {
  pub owner: Owner,
  pub set_viewport: WriteSignal<Size<f32>>,
  pub set_scheme: WriteSignal<ColorScheme>,
  pub set_reduced_motion: WriteSignal<bool>,
  pub set_reduced_transparency: WriteSignal<bool>,
  pub set_high_contrast: WriteSignal<bool>,
  pub set_pointer: WriteSignal<PointerPrecision>,
  pub set_hover: WriteSignal<bool>,
  pub set_safe_area: WriteSignal<Inset<f32>>,
  pub set_hydrated: WriteSignal<bool>,
  pub set_graphics: WriteSignal<bool>,
  pub hydrated: ReadSignal<bool>,
  pub graphics: ReadSignal<bool>,
  pub signals: MediaSignals<f32>,
}

/// Every record the platform holds, the focus, the root definitions and the environment.
pub(crate) struct Records {
  /// The records, by node.
  by_node: NodeMap<Record>,
  /// The token definitions on the root, as last written.
  pub root_definitions: Definitions<TokenValue>,
  /// How many times the root definitions have been written.
  pub root_definition_writes: usize,
  /// The viewport, as set; [`DEFAULT_VIEWPORT`] unless set.
  pub viewport: Option<Size<f32>>,
  /// The body a page is mounted in, once one is.
  pub body: Option<NodeId>,
  /// The platform's color scheme, as set; light unless set.
  pub scheme: Option<ColorScheme>,
  /// The color scheme the app has chosen, overriding the platform's.
  pub chosen_scheme: Option<ColorScheme>,
  /// Whether the reader prefers reduced motion, as set; not unless set.
  pub reduced_motion: Option<bool>,
  /// Whether the reader prefers reduced transparency, as set; not unless set.
  pub reduced_transparency: Option<bool>,
  /// Whether the reader prefers more contrast, as set; not unless set.
  pub high_contrast: Option<bool>,
  /// How precisely the reader's primary pointer points, as set; finely unless set.
  pub pointer: Option<PointerPrecision>,
  /// Whether the reader's primary pointer hovers, as set; it does unless set.
  pub hover: Option<bool>,
  /// The insets the display keeps clear of the viewport's edges, as set; none unless
  /// set.
  pub safe_area: Option<Inset<f32>>,
  /// What moved since the layout last restyled the tree: the resolution generation,
  /// moving whenever the environment, a container's size or a definition is set, and the
  /// nodes the next layout revisits.
  pub restyle: Restyle,
  /// How many node styles the layout has resolved.
  pub style_resolutions: u64,
  /// The motion clock, in seconds; stands still until advanced.
  pub clock: f64,
  /// The document's language, as last set.
  pub language: Option<String>,
  /// The head registrations.
  pub head: Head,
  /// The scheduled actions, each due at a clock reading.
  pub timers: Timers<Box<dyn FnOnce()>>,
  /// The clipboard actions performed, in order, until taken.
  pub clipboard: Vec<ClipboardAction>,
  /// What each clipboard action answers when asked; enabled unless set.
  pub availability: FxHashMap<ClipboardAction, Availability>,
  /// Whether the render stands hydrated, as set; hydrated unless set.
  pub hydrated: Option<bool>,
  /// Whether a painted box shows as pixels, as set; shown unless set.
  pub graphics: Option<bool>,
  /// The environment's signals, once first asked for.
  media: Option<MediaState>,
  /// The rectangles of the last layout.
  pub frames: Frames,
}

impl Default for Records {
  fn default() -> Self {
    Self {
      by_node: NodeMap::new(),
      root_definitions: Definitions::from(Vec::new()),
      root_definition_writes: 0,
      viewport: None,
      body: None,
      scheme: None,
      chosen_scheme: None,
      reduced_motion: None,
      reduced_transparency: None,
      high_contrast: None,
      pointer: None,
      hover: None,
      safe_area: None,
      restyle: Restyle::new(),
      style_resolutions: 0,
      clock: 0.,
      language: None,
      head: Head::default(),
      timers: Timers::new(),
      clipboard: Vec::new(),
      availability: FxHashMap::default(),
      hydrated: None,
      graphics: None,
      media: None,
      frames: Frames::default(),
    }
  }
}

impl Records {
  /// Stores `record` as the record of `node`.
  pub(crate) fn insert(&mut self, node: NodeId, mut record: Record) {
    record.motion.runner = Motion::started_at(self.clock);
    self.restyle.move_structure();

    let _replaced = self.by_node.insert(node, record);
  }

  /// The record of `node`, if it is live.
  #[inline(always)]
  pub(crate) fn record(&self, node: NodeId) -> Option<&Record> {
    self.by_node.get(node)
  }

  /// The record of `node`, mutably, if it is live.
  #[inline(always)]
  pub(crate) fn record_mut(&mut self, node: NodeId) -> Option<&mut Record> {
    self.by_node.get_mut(node)
  }

  /// Releases the record of `node`.
  pub(crate) fn destroy(&mut self, node: NodeId) {
    let _released = self.by_node.remove(node);

    self.restyle.move_structure();
    self.restyle.read_interaction(node, false);
  }

  /// The color scheme in force: the app's choice, else the platform's.
  #[inline(always)]
  pub(crate) fn scheme_in_force(&self) -> ColorScheme {
    self
      .chosen_scheme
      .or(self.scheme)
      .unwrap_or(ColorScheme::Light)
  }

  /// The environment the platform reports, as set.
  pub(crate) fn environment(&self) -> Environment<f32> {
    Environment {
      viewport: self.viewport.unwrap_or(DEFAULT_VIEWPORT),
      scheme: self.scheme_in_force(),
      reduced_motion: self.reduced_motion.unwrap_or(false),
      reduced_transparency: self.reduced_transparency.unwrap_or(false),
      high_contrast: self.high_contrast.unwrap_or(false),
      pointer: self.pointer.unwrap_or_default(),
      hover: self.hover.unwrap_or(true),
      safe_area: self.safe_area.unwrap_or_default(),
    }
  }

  /// The environment's signals, created on first use under a root owner of their own.
  pub(crate) fn media(&mut self) -> &MediaState {
    let viewport = self.viewport.unwrap_or(DEFAULT_VIEWPORT);
    let scheme = self.scheme_in_force();
    let reduced_motion = self.reduced_motion.unwrap_or(false);
    let reduced_transparency = self.reduced_transparency.unwrap_or(false);
    let high_contrast = self.high_contrast.unwrap_or(false);
    let pointer = self.pointer.unwrap_or_default();
    let hover = self.hover.unwrap_or(true);
    let safe_area = self.safe_area.unwrap_or_default();
    let hydrated = self.hydrated.unwrap_or(true);
    let graphics = self.graphics.unwrap_or(true);

    self.media.get_or_insert_with(|| {
      let owner = Owner::new();

      owner.enter(|| {
        let (signals, writers) = MediaSignals::new(Environment {
          viewport,
          scheme,
          reduced_motion,
          reduced_transparency,
          high_contrast,
          pointer,
          hover,
          safe_area,
        });
        let (hydrated, set_hydrated) = Signal::new(hydrated).split();
        let (graphics, set_graphics) = Signal::new(graphics).split();

        MediaState {
          owner,
          set_viewport: writers.viewport,
          set_scheme: writers.scheme,
          set_reduced_motion: writers.reduced_motion,
          set_reduced_transparency: writers.reduced_transparency,
          set_high_contrast: writers.high_contrast,
          set_pointer: writers.pointer,
          set_hover: writers.hover,
          set_safe_area: writers.safe_area,
          set_hydrated,
          set_graphics,
          hydrated,
          graphics,
          signals,
        }
      })
    })
  }

  /// Every property a resolution has observed, by node, for a frame to judge again.
  #[must_use]
  pub(crate) fn tracked(&self) -> Vec<(NodeId, Vec<PropertyKey<WebProperty>>)> {
    self
      .by_node
      .iter()
      .filter_map(|(node, record)| {
        let motion = &record.motion;

        (!motion.targets.is_empty()).then(|| (node, motion.targets.keys().copied().collect()))
      })
      .collect()
  }

  /// Forgets everything and hands back the owner of the environment's signals, for
  /// the caller to dispose once the records are released.
  pub(crate) fn reset(&mut self) -> Option<Owner> {
    let media = self.media.take();

    *self = Self::default();

    media.map(|media| media.owner)
  }
}
