//! What the records keep per node: its kind with the facts its creation carried, its
//! style kept as a record, its resolved style and motion, its text and slots, and what
//! it reports of its size.

use std::rc::Rc;

use rustc_hash::FxHashMap;
use skene_foundation::{
  BoxFacts,
  BoxStyle,
  Color,
  CompiledStyle,
  ContainerName,
  DragAxes,
  Interaction,
  Layout,
  NativeElement,
  NodeId,
  Placement,
  Plain,
  Record as StyleRecord,
  Role,
  RunFacts,
  RunStyle,
  Size,
  Slot,
  terminal::TerminalStyle,
};

/// A box-like node's motion on the terminal's frame clock.
pub(crate) type BoxMotion = skene_layout::runtime::BoxMotion<i32, TerminalStyle>;

/// A run's motion on the terminal's frame clock.
pub(crate) type RunMotion = skene_layout::runtime::RunMotion<TerminalStyle>;

/// A box's style resolved against live state.
pub(crate) type PlainBox = BoxStyle<i32, Plain, TerminalStyle>;

/// A run's style resolved against live state.
pub(crate) type PlainRun = RunStyle<Plain, TerminalStyle>;

/// What a derived leaf last wrote into a node's slot; see [`skene_foundation::SlotLeaf`].
pub(crate) type SlotLeaf = skene_foundation::SlotLeaf<i32, TerminalStyle>;

/// A node's style and motion as the shared half applies it, its record kept at
/// [`Keep::Everything`](skene_foundation::Keep::Everything), which every restyle resolves
/// through [`ResolutionContext::resolve`](skene_foundation::ResolutionContext::resolve).
pub(crate) type KeptStyle = CompiledStyle<i32, TerminalStyle>;

/// A box's style resolved under one generation and interaction: the style presented,
/// with its motion laid over, and the target it resolved to.
pub(crate) struct BoxCache {
  /// The resolution generation it was resolved under.
  pub(crate) generation: u64,
  /// The interaction state it was resolved under.
  pub(crate) interaction: Interaction,
  /// The style laid out and painted: the target with every running tween sampled.
  pub(crate) style: PlainBox,
  /// The style the resolution produced, which a change is judged against.
  pub(crate) target: PlainBox,
  /// The keyframes resolved at the node, reused across the frames of a running motion.
  pub(crate) keyframes: Vec<skene_foundation::Animation<PlainBox>>,
}

/// A run's style resolved under one generation and interaction, with the per-child
/// layout properties a wrapper annotated the run with.
pub(crate) struct RunCache {
  /// The resolution generation it was resolved under.
  pub(crate) generation: u64,
  /// The interaction state it was resolved under.
  pub(crate) interaction: Interaction,
  /// The style painted: the target with every running tween sampled.
  pub(crate) style: PlainRun,
  /// The style the resolution produced, which a change is judged against.
  pub(crate) target: PlainRun,
  /// The per-child layout properties a wrapper annotated the run with.
  pub(crate) layout: PlainBox,
  /// The keyframes resolved at the node, reused across the frames of a running motion.
  pub(crate) keyframes: Vec<skene_foundation::Animation<PlainRun>>,
}

/// A box-like node's facts, its kept style, its resolution and its motion.
pub(crate) struct BoxRecord {
  /// The role the node was created with.
  pub(crate) role: Option<Role>,
  /// The axes a press's pointer drags the node along; `None` for a node no press drags.
  pub(crate) drag: Option<DragAxes>,
  /// The facts of the box the node was created with.
  pub(crate) facts: BoxFacts,
  /// The node's style and motion, kept whole as the shared half applied it; empty until
  /// it does.
  pub(crate) kept: KeptStyle,
  /// The style last resolved and presented, once a frame resolved it.
  pub(crate) cache: Option<BoxCache>,
  /// The node's running transitions and animations.
  pub(crate) motion: BoxMotion,
}

impl BoxRecord {
  /// The record of a box created stating `role`, dragged along `drag`, from `facts`,
  /// its style setting nothing until the shared half applies it.
  pub(crate) fn new(role: Option<Role>, drag: Option<DragAxes>, facts: BoxFacts) -> Self {
    Self {
      role,
      drag,
      facts,
      kept: StyleRecord::new().into_kept(),
      cache: None,
      motion: BoxMotion::default(),
    }
  }

  /// Marks the resolution stale while keeping what the node presents, so the next
  /// frame resolves again and a moved value starts from where the eye left off.
  #[inline(always)]
  pub(crate) const fn mark_stale(&mut self) {
    if let Some(cache) = &mut self.cache {
      cache.generation = 0;
    }
  }

  /// The resolved style, once a frame resolved it.
  #[inline(always)]
  pub(crate) fn style(&self) -> Option<&PlainBox> {
    self.cache.as_ref().map(|cache| &cache.style)
  }
}

/// What a record is, with everything its creation carried.
pub(crate) enum Kind {
  /// A box.
  Box(BoxRecord),
  /// A control: a box a native element may stand in for.
  Control {
    /// The native element the control stands in; `None` for one its view draws.
    native: Option<NativeElement>,
    /// The name the control was created with.
    label: Option<String>,
    /// The control's own box.
    inner: BoxRecord,
  },
  /// An overlay: a box shown above the app while open.
  Overlay {
    /// The overlay's own box, carrying the overlay's role.
    inner: BoxRecord,
    /// Where the overlay sits, as its last placement content said.
    placement: Placement,
    /// The backdrop color painted over the app while open, if any.
    backdrop: Option<Color>,
    /// Whether the overlay is open.
    open: bool,
    /// The cell the overlay's top-start corner is pinned to, if it is, as its last
    /// placement content said.
    position: Option<(i64, i64)>,
  },
  /// A text run.
  Run {
    /// The facts the run was created with.
    facts: RunFacts,
    /// The path a link run navigates to, as its last destination content said.
    destination: Option<String>,
    /// The run's style, then the per-child layout a wrapper annotated it with, and its
    /// motion, kept whole as the shared half applied them; empty until it does.
    kept: KeptStyle,
    /// The style last resolved and presented, once a frame resolved it.
    cache: Option<RunCache>,
    /// The run's running transitions and animations.
    motion: RunMotion,
  },
}

impl Kind {
  /// The record of a run created from `facts`, its style setting nothing until the
  /// shared half applies it.
  pub(crate) fn run(facts: RunFacts) -> Self {
    Self::Run {
      facts,
      destination: None,
      kept: StyleRecord::new().into_kept(),
      cache: None,
      motion: RunMotion::default(),
    }
  }

  /// The node's kept style, replaced by what the shared half applies: a box's, a
  /// control's, an overlay's or a run's.
  pub(crate) const fn kept_mut(&mut self) -> Option<&mut KeptStyle> {
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
  pub(crate) fn forget_presentation(&mut self) {
    match self {
      Self::Run { cache, motion, .. } => {
        *cache = None;
        *motion = RunMotion::default();
      }
      kind => {
        if let Some(inner) = kind.box_record_mut() {
          inner.cache = None;
          inner.motion = BoxMotion::default();
        }
      }
    }
  }

  /// Marks the node's resolution stale while keeping what it presents, so the next
  /// frame resolves it again and a moved value starts from where the eye left off.
  pub(crate) const fn mark_stale(&mut self) {
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
  pub(crate) fn resolved_under(&self) -> Option<(u64, Interaction)> {
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

  /// The box record of a box, a control or an overlay.
  #[inline(always)]
  pub(crate) const fn box_record(&self) -> Option<&BoxRecord> {
    match self {
      Self::Box(inner) | Self::Control { inner, .. } | Self::Overlay { inner, .. } => Some(inner),
      Self::Run { .. } => None,
    }
  }

  /// The box record of a box, a control or an overlay, mutably.
  #[inline(always)]
  pub(crate) const fn box_record_mut(&mut self) -> Option<&mut BoxRecord> {
    match self {
      Self::Box(inner) | Self::Control { inner, .. } | Self::Overlay { inner, .. } => Some(inner),
      Self::Run { .. } => None,
    }
  }

  /// The resolved style of a box-like record, once a frame resolved it.
  #[inline(always)]
  pub(crate) fn box_style(&self) -> Option<&PlainBox> {
    self.box_record().and_then(BoxRecord::style)
  }
}

/// One realized node.
pub(crate) struct Record {
  /// What the node is, with the facts it was created from, its resolution and its motion.
  pub(crate) kind: Kind,
  /// The scene node the record realizes, once bound.
  pub(crate) node: Option<NodeId>,
  /// A run's text, as last set.
  pub(crate) text: String,
  /// The node's slot values, as last written.
  pub(crate) slots: FxHashMap<Slot, SlotLeaf>,
  /// The reports of the box's size.
  pub(crate) observers: Vec<Rc<dyn Fn(Size<i32>)>>,
  /// The size last reported to the observers.
  pub(crate) reported: Option<Size<i32>>,
  /// Whether a tween or an animation of the node was still running at the last
  /// restyle, so the next frame resolves it again.
  pub(crate) in_motion: bool,
  /// Whether the node was out of the display at the last restyle: inside a closed
  /// overlay, or detached from the root.
  pub(crate) hidden: bool,
}

impl Record {
  pub(super) fn new(kind: Kind) -> Self {
    Self {
      kind,
      node: None,
      text: String::new(),
      slots: FxHashMap::default(),
      observers: Vec::new(),
      reported: None,
      in_motion: false,
      hidden: false,
    }
  }

  /// The layout mode of a box-like record.
  #[inline(always)]
  pub(crate) fn layout(&self) -> Option<Layout> {
    self.kind.box_record().map(|inner| inner.facts.layout)
  }

  /// The name the record publishes for container queries.
  #[inline(always)]
  pub(crate) fn container(&self) -> Option<ContainerName> {
    self
      .kind
      .box_record()
      .and_then(|inner| inner.facts.container)
  }

  /// Whether the record's box is hidden by its style: it and its content draw nothing
  /// and take neither the pointer nor focus.
  #[inline(always)]
  pub(crate) fn is_invisible(&self) -> bool {
    self.kind.box_style().and_then(|style| style.hidden) == Some(true)
  }

  /// Whether the record is an overlay.
  #[inline(always)]
  pub(crate) const fn is_overlay(&self) -> bool {
    matches!(self.kind, Kind::Overlay { .. })
  }

  /// Whether the record is an open overlay.
  #[inline(always)]
  pub(crate) const fn is_open_overlay(&self) -> bool {
    matches!(self.kind, Kind::Overlay { open: true, .. })
  }
}
