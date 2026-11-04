//! What the drawn runtime keeps per node: its kind with the facts its creation carried,
//! its style kept as a record, its resolved style and motion, its text and slots, and
//! what it reports of its size.

use std::{fmt, rc::Rc};

use rustc_hash::FxHashMap;
use skene_foundation::{
  Animation,
  BoxFacts,
  BoxStyle,
  Color,
  CompiledStyle,
  ContainerName,
  DragAxes,
  Interaction,
  Layout,
  NativeElement,
  Placement,
  Plain,
  Point,
  Realization,
  Record as StyleRecord,
  Rect,
  Role,
  RunFacts,
  RunStyle,
  Size,
  Slot,
};

use super::{BoxMotion, Drawn, RunMotion};

/// A box's style on the platform `P`, resolved against live state.
pub type PlainBox<P> = BoxStyle<<P as Realization>::Scalar, Plain, <P as Realization>::StyleExt>;

/// A run's style on the platform `P`, resolved against live state.
pub type PlainRun<P> = RunStyle<Plain, <P as Realization>::StyleExt>;

/// A node's style and motion as the shared half applies it on the platform `P`, its
/// record kept at [`Keep::Everything`](skene_foundation::Keep::Everything), which every
/// restyle resolves through
/// [`ResolutionContext::resolve`](skene_foundation::ResolutionContext::resolve).
pub type KeptStyle<P> = CompiledStyle<<P as Realization>::Scalar, <P as Realization>::StyleExt>;

/// What a derived leaf last wrote into a node's slot on the platform `P`; see
/// [`skene_foundation::SlotLeaf`].
pub type SlotLeaf<P> =
  skene_foundation::SlotLeaf<<P as Realization>::Scalar, <P as Realization>::StyleExt>;

/// A report of a box's size on the platform `P`.
pub type Observer<P> = Rc<dyn Fn(Size<<P as Realization>::Scalar>)>;

/// A box's style resolved under one generation and interaction: the style presented,
/// with its motion laid over, and the target it resolved to.
#[derive(Debug)]
#[must_use]
pub struct BoxCache<P: Drawn> {
  /// The resolution generation it was resolved under.
  pub generation: u64,
  /// The interaction state it was resolved under.
  pub interaction: Interaction,
  /// The style laid out and painted: the target with every running tween sampled.
  pub style: PlainBox<P>,
  /// The style the resolution produced, which a change is judged against.
  pub target: PlainBox<P>,
  /// The keyframes resolved at the node, reused across the frames of a running motion.
  pub keyframes: Vec<Animation<PlainBox<P>>>,
}

/// A run's style resolved under one generation and interaction, with the per-child
/// layout properties a wrapper annotated the run with.
#[derive(Debug)]
#[must_use]
pub struct RunCache<P: Drawn> {
  /// The resolution generation it was resolved under.
  pub generation: u64,
  /// The interaction state it was resolved under.
  pub interaction: Interaction,
  /// The style painted: the target with every running tween sampled.
  pub style: PlainRun<P>,
  /// The style the resolution produced, which a change is judged against.
  pub target: PlainRun<P>,
  /// The per-child layout properties a wrapper annotated the run with.
  pub layout: PlainBox<P>,
  /// The keyframes resolved at the node, reused across the frames of a running motion.
  pub keyframes: Vec<Animation<PlainRun<P>>>,
}

/// A box-like node's facts, its kept style, its resolution and its motion.
#[derive(Debug)]
#[must_use]
pub struct BoxRecord<P: Drawn> {
  /// The role the node was created with.
  pub role: Option<Role>,
  /// The axes a press's pointer drags the node along; `None` for a node no press drags.
  pub drag: Option<DragAxes>,
  /// The facts of the box the node was created with.
  pub facts: BoxFacts,
  /// The node's style and motion, kept whole as the shared half applied it; empty until
  /// it does.
  pub kept: KeptStyle<P>,
  /// The style last resolved and presented, once a frame resolved it.
  pub cache: Option<BoxCache<P>>,
  /// The node's running transitions and animations.
  pub motion: BoxMotion<P::Scalar, P::StyleExt>,
}

impl<P: Drawn> BoxRecord<P> {
  /// The record of a box created stating `role`, dragged along `drag`, from `facts`,
  /// its style setting nothing until the shared half applies it.
  pub fn new(role: Option<Role>, drag: Option<DragAxes>, facts: BoxFacts) -> Self {
    Self {
      role,
      drag,
      facts,
      kept: StyleRecord::new().into_kept(),
      cache: None,
      motion: BoxMotion::new(),
    }
  }

  /// Marks the resolution stale while keeping what the node presents, so the next
  /// frame resolves again and a moved value starts from where the eye left off.
  #[inline(always)]
  pub const fn mark_stale(&mut self) {
    if let Some(cache) = &mut self.cache {
      cache.generation = 0;
    }
  }

  /// The resolved style, once a frame resolved it.
  #[inline(always)]
  pub fn style(&self) -> Option<&PlainBox<P>> {
    self.cache.as_ref().map(|cache| &cache.style)
  }
}

/// What a record is, with everything its creation carried.
#[derive(Debug)]
#[must_use]
pub enum Kind<P: Drawn> {
  /// A box.
  Box(BoxRecord<P>),
  /// A control: a box a native element may stand in for.
  Control {
    /// The native element the control stands in; `None` for one its view draws.
    native: Option<NativeElement>,
    /// The name the control was created with.
    label: Option<String>,
    /// The control's own box.
    inner: BoxRecord<P>,
  },
  /// An overlay: a box shown above the app while open.
  Overlay {
    /// The overlay's own box, carrying the overlay's role.
    inner: BoxRecord<P>,
    /// Where the overlay sits, as its last placement content said.
    placement: Placement,
    /// The backdrop color painted over the app while open, if any.
    backdrop: Option<Color>,
    /// Whether the overlay is open.
    open: bool,
    /// The point the overlay's top-start corner is pinned to, if it is, in the units a
    /// pointer event reports, as its last placement content said.
    position: Option<Point<f32>>,
  },
  /// A text run.
  Run {
    /// The facts the run was created with.
    facts: RunFacts,
    /// The path a link run navigates to, as its last destination content said.
    destination: Option<String>,
    /// The run's style, then the per-child layout a wrapper annotated it with, and its
    /// motion, kept whole as the shared half applied them; empty until it does.
    kept: KeptStyle<P>,
    /// The style last resolved and presented, once a frame resolved it.
    cache: Option<RunCache<P>>,
    /// The run's running transitions and animations.
    motion: RunMotion<P::StyleExt>,
  },
}

impl<P: Drawn> Kind<P> {
  /// The record of a run created from `facts`, its style setting nothing until the
  /// shared half applies it.
  pub fn run(facts: RunFacts) -> Self {
    Self::Run {
      facts,
      destination: None,
      kept: StyleRecord::new().into_kept(),
      cache: None,
      motion: RunMotion::new(),
    }
  }

  /// The node's kept style, replaced by what the shared half applies: a box's, a
  /// control's, an overlay's or a run's.
  pub const fn kept_mut(&mut self) -> Option<&mut KeptStyle<P>> {
    match self {
      Self::Run { kept, .. } => Some(kept),
      kind => match kind.box_record_mut() {
        Some(inner) => Some(&mut inner.kept),
        None => None,
      },
    }
  }

  /// Forgets what the node presented and the motion it ran, so it presents afresh: its
  /// animations from their start, and no transition into its style.
  pub fn forget_presentation(&mut self) {
    match self {
      Self::Run { cache, motion, .. } => {
        *cache = None;
        *motion = RunMotion::new();
      }
      kind => {
        if let Some(inner) = kind.box_record_mut() {
          inner.cache = None;
          inner.motion = BoxMotion::new();
        }
      }
    }
  }

  /// Marks the node's resolution stale while keeping what it presents, so the next
  /// frame resolves it again and a moved value starts from where the eye left off.
  pub const fn mark_stale(&mut self) {
    match self {
      Self::Run {
        cache: Some(cache), ..
      } => cache.generation = 0,
      kind => {
        if let Some(inner) = kind.box_record_mut() {
          inner.mark_stale();
        }
      }
    }
  }

  /// The generation and the interaction the node's style was resolved under, once a
  /// frame resolved it.
  pub fn resolved_under(&self) -> Option<(u64, Interaction)> {
    match self {
      Self::Run { cache, .. } => cache
        .as_ref()
        .map(|cache| (cache.generation, cache.interaction)),
      kind => kind
        .box_record()
        .and_then(|inner| inner.cache.as_ref())
        .map(|cache| (cache.generation, cache.interaction)),
    }
  }

  /// Whether the node is a box a view taking decoration created: a container of its
  /// own, laid out by a layout of its own.
  #[inline(always)]
  pub const fn is_decorated(&self) -> bool {
    match self.box_record() {
      Some(inner) => inner.facts.decorated,
      None => false,
    }
  }

  /// Whether the node is a control realized as a text entry, single-line or not.
  #[inline(always)]
  pub const fn is_text_entry(&self) -> bool {
    match self {
      Self::Control {
        native: Some(native),
        ..
      } => native.is_text_entry(),
      _ => false,
    }
  }

  /// The box record of a box, a control or an overlay.
  #[inline(always)]
  pub const fn box_record(&self) -> Option<&BoxRecord<P>> {
    match self {
      Self::Box(inner) | Self::Control { inner, .. } | Self::Overlay { inner, .. } => Some(inner),
      Self::Run { .. } => None,
    }
  }

  /// The box record of a box, a control or an overlay, mutably.
  #[inline(always)]
  pub const fn box_record_mut(&mut self) -> Option<&mut BoxRecord<P>> {
    match self {
      Self::Box(inner) | Self::Control { inner, .. } | Self::Overlay { inner, .. } => Some(inner),
      Self::Run { .. } => None,
    }
  }

  /// The resolved style of a box-like record, once a frame resolved it.
  #[inline(always)]
  pub fn box_style(&self) -> Option<&PlainBox<P>> {
    self.box_record().and_then(BoxRecord::style)
  }

  /// The resolved style of a run, once a frame resolved it.
  #[inline(always)]
  pub const fn run_style(&self) -> Option<&PlainRun<P>> {
    match self {
      Self::Run {
        cache: Some(cache), ..
      } => Some(&cache.style),
      _ => None,
    }
  }
}

/// One realized node.
#[must_use]
pub struct Record<P: Drawn> {
  /// What the node is, with the facts it was created from, its resolution and its motion.
  pub kind: Kind<P>,
  /// A run's text, as last set.
  pub text: String,
  /// The node's slot values, as last written.
  pub slots: FxHashMap<Slot, SlotLeaf<P>>,
  /// The reports of the box's size.
  pub observers: Vec<Observer<P>>,
  /// The size last reported to the observers.
  pub reported: Option<Size<P::Scalar>>,
  /// Whether a tween or an animation of the node was still running at the last
  /// presentation ([`Runtime::write_styles`](super::Runtime::write_styles)), so the next one
  /// presents it again.
  pub in_motion: bool,
  /// Whether the node was out of the display at the last presentation: inside a closed
  /// overlay, or detached from the root.
  pub hidden: bool,
  /// The box's scroll offsets as last written, which a read clamps to what the last
  /// layout allows ([`Runtime::scroll_offset`](super::Runtime::scroll_offset)); a text
  /// entry's window is the platform's own ([`Drawn::entry_scroll`]).
  pub scroll: Point<f32>,
  /// What the node's paint covered in the window at the last paint that recorded the
  /// covers ([`Runtime::take_damage`](super::Runtime::take_damage)); `None` where it
  /// covered nothing.
  pub painted: Option<Rect<f32>>,
  /// The platform's own state of the node ([`Drawn::Node`]).
  pub platform: P::Node,
}

impl<P: Drawn> Record<P> {
  /// The record of a node realized as `kind`, the platform's own state of it `platform`:
  /// no text, slot, observer or report yet, at rest, unscrolled, in the display and
  /// unpainted.
  pub fn new(kind: Kind<P>, platform: P::Node) -> Self {
    Self {
      kind,
      text: String::new(),
      slots: FxHashMap::default(),
      observers: Vec::new(),
      reported: None,
      in_motion: false,
      hidden: false,
      scroll: Point::ZERO,
      painted: None,
      platform,
    }
  }

  /// The layout mode of a box-like record.
  #[inline(always)]
  pub fn layout(&self) -> Option<Layout> {
    self.kind.box_record().map(|inner| inner.facts.layout)
  }

  /// The name the record publishes for container queries.
  #[inline(always)]
  pub fn container(&self) -> Option<ContainerName> {
    self
      .kind
      .box_record()
      .and_then(|inner| inner.facts.container)
  }

  /// Whether the record's box is hidden by its style: it and its content draw nothing
  /// and take neither the pointer nor focus.
  #[inline(always)]
  pub fn is_invisible(&self) -> bool {
    self.kind.box_style().and_then(|style| style.hidden) == Some(true)
  }

  /// Whether the record is an overlay.
  #[inline(always)]
  pub const fn is_overlay(&self) -> bool {
    matches!(self.kind, Kind::Overlay { .. })
  }

  /// Whether the record is an open overlay.
  #[inline(always)]
  pub const fn is_open_overlay(&self) -> bool {
    matches!(self.kind, Kind::Overlay { open: true, .. })
  }
}

impl<P: Drawn + fmt::Debug> fmt::Debug for Record<P> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Record")
      .field("kind", &self.kind)
      .field("text", &self.text)
      .field("slots", &self.slots)
      .field("observers", &self.observers.len())
      .field("reported", &self.reported)
      .field("in_motion", &self.in_motion)
      .field("hidden", &self.hidden)
      .field("scroll", &self.scroll)
      .field("painted", &self.painted)
      .field("platform", &self.platform)
      .finish()
  }
}
