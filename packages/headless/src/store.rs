//! The record store: what the platform keeps per node, and the environment it reports.

use std::rc::Rc;

use rustc_hash::FxHashMap;
use skene_common::gui::{Head, Painter};
use skene_core::{
  Availability,
  BoxProps,
  ClipboardAction,
  ColorScheme,
  ContainerName,
  ContainerSizes,
  ControlProps,
  ControlState,
  Definitions,
  EventKind,
  MediaSignals,
  NoProperty,
  NodeId,
  OverlayProps,
  Point,
  PropertyKey,
  RunProps,
  Size,
  Slot,
  TextSelection,
  Timer,
  interpolate::Tween,
};
use skene_reactive::{Owner, ReadSignal, Signal, WriteSignal};

use crate::{Handle, Headless, SlotLeaf, SlotValue, TokenValue};

/// The viewport the platform reports until [`set_viewport`](crate::set_viewport) is called.
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

/// The motion a node runs: the value each resolved property is heading for, the tween
/// of each property in flight, and the clock reading the node was created at.
#[derive(Default)]
pub(crate) struct Motion {
  /// The node the record realizes, once a resolution named it.
  pub node: Option<NodeId>,
  /// The value last resolved per property, which a change is judged against.
  pub targets: FxHashMap<PropertyKey<NoProperty>, SlotValue>,
  /// The tween of each property in flight.
  pub tweens: FxHashMap<PropertyKey<NoProperty>, Tween<SlotValue>>,
  /// The clock reading the node was created at, which its animations count from.
  pub started: f64,
}

/// One realized node.
// Each flag is an independent fact a platform call recorded.
pub(crate) struct Record {
  pub kind: Kind,
  pub text: String,
  pub value: String,
  /// The options a platform-drawn picker offers, each a value and its name.
  pub options: Vec<(String, String)>,
  pub selection: TextSelection,
  pub checked: bool,
  pub open: bool,
  pub anchor: Option<Handle>,
  pub position: Option<Point<f32>>,
  pub control_state: ControlState,
  pub hovered: bool,
  pub slots: FxHashMap<Slot, SlotLeaf>,
  pub definitions: Option<Definitions<TokenValue>>,
  pub listened: Vec<EventKind>,
  pub children: Vec<Handle>,
  pub size_observers: Vec<Rc<dyn Fn(Size<f32>)>>,
  pub painter: Option<Rc<Painter>>,
  pub motion: Motion,
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

  /// The box props of a box or a control, mutably.
  pub(crate) fn box_props_mut(&mut self) -> Option<&mut BoxProps<Headless>> {
    match self {
      Self::Box(props) => Some(props),
      Self::Control(props) => Some(&mut props.box_props),
      Self::Overlay(props) => Some(&mut props.box_props),
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
      anchor: None,
      position: None,
      control_state: ControlState::DEFAULT,
      hovered: false,
      slots: FxHashMap::default(),
      definitions: None,
      listened: Vec::new(),
      children: Vec::new(),
      size_observers: Vec::new(),
      painter: None,
      motion: Motion::default(),
    }
  }
}

struct Entry {
  generation: u32,
  record: Option<Record>,
}

/// The environment as signals, with the write halves the seams drive.
pub(crate) struct MediaState {
  pub owner: Owner,
  pub set_viewport: WriteSignal<Size<f32>>,
  pub set_scheme: WriteSignal<ColorScheme>,
  pub set_reduced_motion: WriteSignal<bool>,
  pub set_hydrated: WriteSignal<bool>,
  pub set_graphics: WriteSignal<bool>,
  pub hydrated: ReadSignal<bool>,
  pub graphics: ReadSignal<bool>,
  pub signals: MediaSignals<f32>,
}

/// Every record the platform holds, the focus, the root definitions and the environment.
pub(crate) struct Store {
  entries: Vec<Entry>,
  free: Vec<u32>,
  pub focused: Option<Handle>,
  /// Whether focus shows while held: set by a key, cleared by a pointer press.
  pub focus_visible: bool,
  pub root_definitions: Definitions<TokenValue>,
  pub root_definition_writes: usize,
  pub container_sizes: FxHashMap<ContainerName, Size<f32>>,
  pub viewport: Option<Size<f32>>,
  pub scheme: Option<ColorScheme>,
  pub chosen_scheme: Option<ColorScheme>,
  pub reduced_motion: Option<bool>,
  /// The motion clock, in seconds; stands still until advanced.
  pub clock: f64,
  pub language: Option<String>,
  pub head: Head,
  /// The scheduled actions, each with the clock reading it is due at.
  pub timers: Vec<Scheduled>,
  pub next_timer: u64,
  /// The clipboard actions performed, in order, until taken.
  pub clipboard: Vec<ClipboardAction>,
  /// What each clipboard action answers when asked; enabled unless set.
  pub availability: FxHashMap<ClipboardAction, Availability>,
  /// Whether the render stands hydrated, as set; hydrated unless set.
  pub hydrated: Option<bool>,
  /// Whether a painted box shows as pixels, as set; shown unless set.
  pub graphics: Option<bool>,
  media: Option<MediaState>,
}

/// A scheduled action: the clock reading it is due at, its timer, and the action.
pub(crate) type Scheduled = (f64, Timer, Box<dyn FnOnce()>);

impl Default for Store {
  fn default() -> Self {
    Self {
      entries: Vec::new(),
      free: Vec::new(),
      focused: None,
      focus_visible: true,
      root_definitions: Definitions::from(Vec::new()),
      root_definition_writes: 0,
      container_sizes: FxHashMap::default(),
      viewport: None,
      scheme: None,
      chosen_scheme: None,
      reduced_motion: None,
      clock: 0.,
      language: None,
      head: Head::default(),
      timers: Vec::new(),
      next_timer: 0,
      clipboard: Vec::new(),
      availability: FxHashMap::default(),
      hydrated: None,
      graphics: None,
      media: None,
    }
  }
}

impl Store {
  /// Mints a record and returns its handle.
  pub(crate) fn create(&mut self, mut record: Record) -> Handle {
    record.motion.started = self.clock;

    if let Some(index) = self.free.pop() {
      let entry = &mut self.entries[index as usize];

      entry.record = Some(record);

      return Handle::new(index, entry.generation);
    }

    let index = u32::try_from(self.entries.len()).expect("a store holds at most 2^32 records");

    self.entries.push(Entry {
      generation: 0,
      record: Some(record),
    });

    Handle::new(index, 0)
  }

  /// The record of `handle`, if it is live.
  #[inline(always)]
  pub(crate) fn record(&self, handle: Handle) -> Option<&Record> {
    self
      .entries
      .get(handle.index as usize)
      .filter(|entry| entry.generation == handle.generation)
      .and_then(|entry| entry.record.as_ref())
  }

  /// The record of `handle`, mutably, if it is live.
  #[inline(always)]
  pub(crate) fn record_mut(&mut self, handle: Handle) -> Option<&mut Record> {
    self
      .entries
      .get_mut(handle.index as usize)
      .filter(|entry| entry.generation == handle.generation)
      .and_then(|entry| entry.record.as_mut())
  }

  /// Releases the record of `handle`, retiring the handle.
  pub(crate) fn destroy(&mut self, handle: Handle) {
    let Some(entry) = self
      .entries
      .get_mut(handle.index as usize)
      .filter(|entry| entry.generation == handle.generation)
    else {
      return;
    };

    if entry.record.take().is_none() {
      return;
    }

    entry.generation = entry.generation.wrapping_add(1);
    self.free.push(handle.index);

    if self.focused == Some(handle) {
      self.focused = None;
    }
  }

  /// The color scheme in force: the app's choice, else the platform's.
  #[inline(always)]
  pub(crate) fn scheme_in_force(&self) -> ColorScheme {
    self
      .chosen_scheme
      .or(self.scheme)
      .unwrap_or(ColorScheme::Light)
  }

  /// The environment's signals, created on first use under a root owner of their own.
  pub(crate) fn media(&mut self) -> &MediaState {
    let viewport = self.viewport.unwrap_or(DEFAULT_VIEWPORT);
    let scheme = self.scheme_in_force();
    let reduced_motion = self.reduced_motion.unwrap_or(false);
    let hydrated = self.hydrated.unwrap_or(true);
    let graphics = self.graphics.unwrap_or(true);

    self.media.get_or_insert_with(|| {
      let owner = Owner::new();

      owner.enter(|| {
        let (viewport, set_viewport) = Signal::opaque(viewport).split();
        let (scheme, set_scheme) = Signal::opaque(scheme).split();
        let (reduced_motion, set_reduced_motion) = Signal::opaque(reduced_motion).split();
        let (hydrated, set_hydrated) = Signal::opaque(hydrated).split();
        let (graphics, set_graphics) = Signal::opaque(graphics).split();

        MediaState {
          owner,
          set_viewport,
          set_scheme,
          set_reduced_motion,
          set_hydrated,
          set_graphics,
          hydrated,
          graphics,
          signals: MediaSignals {
            viewport,
            scheme,
            reduced_motion,
            container_sizes: ContainerSizes::new(),
          },
        }
      })
    })
  }

  /// Every property a resolution has observed, by node, for a frame to judge again.
  #[must_use]
  pub(crate) fn tracked(&self) -> Vec<(NodeId, Vec<PropertyKey<NoProperty>>)> {
    self
      .entries
      .iter()
      .filter_map(|entry| {
        let motion = &entry.record.as_ref()?.motion;
        let node = motion.node?;

        (!motion.targets.is_empty()).then(|| (node, motion.targets.keys().copied().collect()))
      })
      .collect()
  }

  /// Forgets everything and hands back the owner of the environment's signals, for
  /// the caller to dispose once the store is released.
  pub(crate) fn reset(&mut self) -> Option<Owner> {
    let media = self.media.take();

    *self = Self::default();

    media.map(|media| media.owner)
  }
}
