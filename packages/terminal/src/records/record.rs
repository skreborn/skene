//! What the records keep per node: its kind with the facts its creation carried, its
//! style kept as a record, its resolved style and motion, and what the frame last
//! measured and painted of it.

use std::rc::Rc;

use rustc_hash::FxHashMap;
use skene_foundation::{
  BoxFacts,
  BoxStyle,
  Color,
  CompiledStyle,
  ContainerName,
  DragAxes,
  Inset,
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
};

use super::{CellRect, field::Field};
use crate::{
  TerminalStyle,
  motion::{BoxMotion, RunMotion},
  paint::Image,
};

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
  Box(BoxRecord),
  Control {
    /// The native element the control stands in; `None` for one its view draws.
    native: Option<NativeElement>,
    /// The name the control was created with.
    label: Option<String>,
    /// The control's own box.
    inner: BoxRecord,
    /// The editing state of a text entry; `None` for every other control.
    field: Option<Box<Field>>,
  },
  Overlay {
    /// The overlay's own box, carrying the overlay's role.
    inner: BoxRecord,
    /// Where the overlay sits.
    placement: Placement,
    /// The backdrop color painted over the app while open, if any.
    backdrop: Option<Color>,
    /// Whether the overlay is open.
    open: bool,
    /// The cell the overlay's top-start corner is pinned to, if it is.
    position: Option<(i64, i64)>,
  },
  Run {
    /// The facts the run was created with.
    facts: RunFacts,
    /// The run's style, then the per-child layout a wrapper annotated it with, and its
    /// motion, kept whole as the shared half applied them; empty until it does.
    kept: KeptStyle,
    /// The style last resolved and presented, once a frame resolved it.
    cache: Option<RunCache>,
    /// The run's running transitions and animations.
    motion: RunMotion,
    /// An inline box's size in cells and rows, as last measured.
    extent: Option<(usize, usize)>,
  },
  Anchor,
  Scope,
}

impl Kind {
  /// The record of a run created from `facts`, its style setting nothing until the
  /// shared half applies it.
  pub(crate) fn run(facts: RunFacts) -> Self {
    Self::Run {
      facts,
      kept: StyleRecord::new().into_kept(),
      cache: None,
      motion: RunMotion::default(),
      extent: None,
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
  #[inline(always)]
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
      Self::Run { .. } | Self::Anchor | Self::Scope => None,
    }
  }

  /// The box record of a box, a control or an overlay, mutably.
  #[inline(always)]
  pub(crate) const fn box_record_mut(&mut self) -> Option<&mut BoxRecord> {
    match self {
      Self::Box(inner) | Self::Control { inner, .. } | Self::Overlay { inner, .. } => Some(inner),
      Self::Run { .. } | Self::Anchor | Self::Scope => None,
    }
  }

  /// The resolved style of a box-like record, once a frame resolved it.
  #[inline(always)]
  pub(crate) fn box_style(&self) -> Option<&PlainBox> {
    self.box_record().and_then(BoxRecord::style)
  }

  /// Whether the record takes part in layout: everything but an anchor or a scope.
  #[inline(always)]
  pub(crate) const fn is_laid_out(&self) -> bool {
    !matches!(self, Self::Anchor | Self::Scope)
  }

  /// The editing state of a text entry.
  #[inline(always)]
  pub(crate) fn field(&self) -> Option<&Field> {
    match self {
      Self::Control { field, .. } => field.as_deref(),
      _ => None,
    }
  }

  /// The editing state of a text entry, mutably.
  #[inline(always)]
  pub(crate) fn field_mut(&mut self) -> Option<&mut Field> {
    match self {
      Self::Control { field, .. } => field.as_deref_mut(),
      _ => None,
    }
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
  /// The scroll offsets, in cells.
  pub(crate) scroll: (u32, u32),
  /// The screen rectangle the node covered at the last paint.
  pub(crate) painted: Option<CellRect>,
  /// The screen rectangle the node covers now, as the frame being painted measured it.
  pub(crate) covers: Option<CellRect>,
  /// Whether a tween or an animation of the node was still running at the last
  /// restyle, so the next frame resolves it again.
  pub(crate) in_motion: bool,
  /// Whether the node was out of the display at the last restyle: inside a closed
  /// overlay, or detached from the root.
  pub(crate) hidden: bool,
  /// The painter a low-level view runs inside the box on every repaint.
  pub(crate) painter: Option<Box<crate::cells::Painter>>,
  /// The picture a painted box draws through the terminal's graphics.
  pub(crate) picture: Option<PictureRecord>,
  /// The box's decoration painted in pixels beneath its cells, where the terminal
  /// shows graphics and the decoration asks for more than cells draw.
  pub(crate) backdrop: Option<Backdrop>,
}

/// A box's decoration as pixels: what it was rendered from, and the image.
pub(crate) struct Backdrop {
  pub(crate) key: BackdropKey,
  pub(crate) image: Rc<Image>,
}

/// What a backdrop is rendered from; a change to any part renders it afresh.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BackdropKey {
  /// The box's columns and the shadow's band.
  pub(crate) columns: u16,
  /// The box's rows and the shadow's band.
  pub(crate) rows: u16,
  /// The cells the shadow reaches down and right with its softening.
  pub(crate) band: u16,
  /// A cell's size in pixels.
  pub(crate) cell_pixels: (u16, u16),
  /// The box's surface; `None` for none.
  pub(crate) background: Option<(u8, u8, u8)>,
  /// The border's ink; `None` for no border or a transparent one.
  pub(crate) border: Option<(u8, u8, u8)>,
  /// The edges the border is drawn on.
  pub(crate) border_edges: Inset<bool>,
  /// The corner radius.
  pub(crate) radius: f32,
  /// The shadow's color as RGBA; `None` for no shadow or a transparent one.
  pub(crate) shadow: Option<[f32; 4]>,
  /// How far the shadow falls down and right, in cells.
  pub(crate) shadow_offset: f32,
  /// How far the shadow's edge softens past its offset, in cells.
  pub(crate) shadow_softening: f32,
}

/// A painted box's picture: its painter, and the image last rendered from it, dropped
/// when the painter is replaced and stale when the box's pixel size differs.
pub(crate) struct PictureRecord {
  pub(crate) painter: Rc<skene_foundation::Painter>,
  pub(crate) image: Option<Rc<Image>>,
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
      scroll: (0, 0),
      painted: None,
      covers: None,
      in_motion: false,
      hidden: false,
      painter: None,
      picture: None,
      backdrop: None,
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
