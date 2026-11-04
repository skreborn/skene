//! The drawn runtime: what every platform Skene draws whole runs beneath its realization.
//!
//! A platform Skene draws whole ([`Drawn`]) keeps one [`Runtime`] in its own state: a
//! [`Record`] per node of the scene, its kind with the facts its creation carried, its
//! kept and resolved style and its motion, beside the root filling the window, the
//! overlays and text entries in build order, what moved since the last restyle and what
//! the next frame lays out and repaints. The runtime reads the scene's structure as the
//! layout and the display see it ([`Runtime::layout_children`], [`Runtime::displayed`]).
//!
//! One pass presents every node whose style may have moved and writes it into the
//! runtime's layout tree ([`Runtime::write_styles`]): it chooses the nodes from what
//! moved since the last pass ([`Restyle`]), resolves each stale one through [`Env`] and
//! caches the resolution under the generation and the interaction it read, and lays its
//! transitions and keyframe animations over it ([`BoxMotion`], [`RunMotion`]) at one
//! reading of the runtime's [`Clock`], counting what it resolved and marking what
//! repaints, what stays in motion and whether a box's reach moved ([`Drawn::reach`]);
//! it then maps each presented node onto its taffy style, a box's border taking room as
//! the platform's layout takes it ([`Drawn::BORDERS`]), and its leaf, a text entry's
//! native extent read through [`Drawn::entry_extent`]. Where the scene's structure
//! moved, every laid-out node is handed its children, anchors and scopes flattened into
//! their content and the root holding the overlays it reaches
//! ([`Runtime::write_children`]), and the layout is computed at the window's size
//! ([`Runtime::compute`]) and read through [`Runtime::tree`].
//!
//! The runtime reads the paragraph structure from its records and the scene: a flow box
//! whose every laid-out child is an inline run lays them out as one paragraph
//! ([`Runtime::is_paragraph`]), its leaf runs visited in order with the groups around
//! each ([`Runtime::visit_runs`]), and a text node's source text names where each of its
//! runs lies in it and in the flow the layout lays out ([`Source`]).
//!
//! The layout measures a paragraph by the runtime's text leaf ([`Measured`]): its runs
//! and inline boxes laid out as one text through the platform's engine
//! ([`Drawn::Engine`]) in the styles the platform gives them ([`Drawn::text_style`]), in
//! whole units where the platform's layout rounds ([`Drawn::ROUNDED`]), made afresh
//! under a new text generation when the engine comes to lay the same text out
//! differently ([`Runtime::remeasure`]); a text entry measures at its native extent.
//!
//! The runtime is the source every node's style resolves through ([`Env`]): it keeps
//! what its platform reports of the window ([`Window`]) and the app's choice of scheme,
//! moved by one writer ([`Runtime::report`]) together with the resolution's generation
//! and the environment's signals, and it calls the platform's fill-ins of its own
//! vocabulary ([`Drawn::finish_box`], [`Drawn::finish_run`]).
//!
//! The runtime keeps what a box scrolls by: its overflow, its scroll offsets clamped at
//! each read to the limits its last layout allows ([`Runtime::scroll_offset`],
//! [`Runtime::scroll_limits`]), and the writes that move them, damaging the box, moving
//! the geometry and notifying its scroll handlers once the runtime is released
//! ([`Runtime::scroll_to`], [`Runtime::scroll_by`]). A text entry whose window the
//! platform scrolls as an offset answers for itself through [`Drawn::entry_scroll`] and
//! [`Drawn::scroll_entry`], its window moving with no notification. A node queued to be
//! scrolled into view ([`Runtime::reveal`]) is shown by the offsets each scroll box
//! around it takes, between its near and its far border ([`Runtime::reveal_offsets`]).
//!
//! The runtime places an open overlay in its window ([`Runtime::overlay_origin`]):
//! pinned at a point and moved back into the window, opening below or above its anchor
//! with a flip where the preferred side runs out of room, or at one of the window's
//! seven placements.
//!
//! The runtime keeps one index of its window's rectangles ([`Runtime::frames`]): every
//! laid-out node's border box in the window, the cover its paint damages, the reach a
//! hit takes it at, the clip what shows of it is cut to and whether it takes hits
//! ([`Frame`]), in paint order, the root's flow and then each open overlay's subtree in
//! the order they opened ([`Layer`]), moved to where the runtime places the overlay. It
//! is built afresh at its first read after a layout, a scroll offset, a box's reach or
//! visibility, an overlay's opening or placement, or the scene's structure moved, beside
//! whether the covers moved since the platform last painted ([`Runtime::covers_moved`]).
//! A run flowing in a paragraph has no entry: it lies in the pieces its paragraph's flow
//! places it in, one per line it reaches, read from that one paragraph as a query asks
//! ([`Runtime::run_rects`]), its rectangle their union and its origin its first piece's
//! corner ([`Runtime::rect`], [`Runtime::origin`]).
//!
//! A point hits the topmost node over that index ([`Runtime::hit`]): each open overlay's
//! layer from the last opened, nothing beneath the topmost modal, then the root's flow,
//! later children above earlier ones, a box that does not clip passing hits on wherever
//! what it holds reaches and one that clips cutting them at its padding box, and a
//! paragraph resolving the point to the run its flow draws there or an inline box. The
//! hit's path is the laid-out chain down to the node from the root, or from the overlay
//! the node lies in.
//!
//! The runtime keeps what the next paint repaints as nodes and the rectangles of the
//! window they cover: each node marked for repaint ([`Runtime::damage`]) and what each
//! destroyed node covered at the last paint, every node's cover at the last paint kept in
//! its record ([`Record::painted`]). The paint takes it as rectangles
//! ([`Runtime::take_damage`]): what each damaged node's paint owner, the paragraph a run
//! flows in ([`Runtime::paint_owner`]), covered then and covers now, and, where the
//! covers moved, what each node whose cover changed covered and covers, every cover then
//! recorded as painted. The platform collapses the rectangles into its own granularity.
//!
//! A drawn platform's realization forwards to the runtime for everything that is no
//! cell's or pixel's: a node's record created ([`Runtime::create`]) and destroyed, its
//! style, slots, state and contents applied ([`Runtime::write`]), its size observed, an
//! overlay opened with the focus move it asks for ([`Runtime::set_overlay_open`]), and a
//! view mounted under the root with the window's autofocus ([`Runtime::mount`]).
//!
//! A record carries the platform's own state of its node ([`Drawn::Node`]), made with
//! the record from the node's spec ([`Drawn::node`]) and dropped with it, and a hook the
//! runtime calls mid-pass is handed it ([`Drawn::entry_extent`], [`Drawn::entry_scroll`],
//! [`Drawn::scroll_entry`]).

mod adapter;
mod environment;
mod flow;
mod frames;
mod motion;
mod overlay;
mod paragraph;
mod record;
mod resolve;
mod restyle;
mod scroll;
mod structure;
mod sync;

use std::cell::{OnceCell, RefCell};

use skene_foundation::{
  Axis,
  BoxFacts,
  BoxStyle,
  ColorScheme,
  Environment,
  Inset,
  Layout,
  NodeId,
  NodeMap,
  NodeSpec,
  Plain,
  Point,
  Realization,
  Rect,
  Resolver,
  RunStyle,
  Size,
  TokenValue,
  interpolate::Interpolate,
  text::{ParagraphStyle, TextEngine, TextStyle},
};

pub use self::{
  adapter::{INLINE_BOX, OverlayFocus},
  environment::{Facts, Pending, Window},
  flow::{Flow, Measured},
  frames::{Frame, Frames, Layer},
  motion::{BoxMotion, Clock, Presented, RunMotion, Sample},
  paragraph::Source,
  record::{
    BoxCache,
    BoxRecord,
    KeptStyle,
    Kind,
    Observer,
    PlainBox,
    PlainRun,
    Record,
    RunCache,
    SlotLeaf,
  },
  resolve::Env,
};
use crate::{Borders, Restyle, Tree};

/// A platform Skene draws whole: its realization keeps the drawn [`Runtime`] in its own
/// state and hands it to the code written once above it.
///
/// Its tokens are the foundation's erased [`TokenValue`] at its scalar, resolved by the
/// runtime rather than kept by a toolkit.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a platform Skene draws whole",
  label = "a drawn platform keeps the drawn runtime in its own state and hands it out through `Drawn::with_runtime`",
  note = "a platform whose toolkit keeps its own tree and resolves its own styles, as the web's does, realizes the seam without the runtime"
)]
pub trait Drawn:
  Realization<TokenValue = TokenValue<<Self as Realization>::Scalar>, Scalar: Interpolate>
{
  /// The engine the platform lays its text out through.
  type Engine: TextEngine;

  /// The platform's own state of a node, kept in the node's record: made by
  /// [`node`](Self::node) as the record is created, the root's its default, and dropped
  /// with the record.
  type Node: Default + std::fmt::Debug;

  /// How a box's border takes room in the platform's layout.
  const BORDERS: Borders;

  /// Whether the platform's layout is rounded to whole units, its text laid out in whole
  /// units, at least one ([`Flow`]).
  const ROUNDED: bool;

  /// Runs `operation` on the platform's runtime, `None` where none is installed for
  /// this thread.
  ///
  /// The platform's state is borrowed while `operation` runs, and a call nested inside
  /// it panics, so `operation` runs nothing of the app's: no handler, and no write whose
  /// effects run before it returns. The platform runs its handlers with nothing
  /// borrowed, so a handler reaches the runtime as any other caller does.
  fn with_runtime<R>(operation: impl FnOnce(&mut Runtime<Self>) -> R) -> Option<R>;

  /// The platform's own state of the node created from `spec`.
  fn node(spec: &NodeSpec) -> Self::Node;

  /// The native extent of the text entry `node` holding the platform's state `own`, its
  /// content box's size, `None` where it is no entry.
  ///
  /// It runs while the runtime is borrowed and reads nothing the runtime holds but `own`.
  fn entry_extent(node: NodeId, own: &Self::Node) -> Option<Size<f32>>;

  /// The window of the text entry `node` holding the platform's state `own`, laid out in
  /// a content box of `content`, as a scroll offset and how far it can scroll: `None`
  /// where it is no entry, or one whose window the platform does not scroll as an
  /// offset.
  ///
  /// It runs while the runtime is borrowed and reads nothing the runtime holds but `own`.
  fn entry_scroll(
    node: NodeId,
    own: &Self::Node,
    content: Size<f32>,
  ) -> Option<(Point<f32>, Size<f32>)>;

  /// Moves the window of the text entry `node` holding the platform's state `own`, laid
  /// out in a content box of `content`, to the scroll offset `offset`, clamped to how far
  /// it can scroll; run only for an entry [`entry_scroll`](Self::entry_scroll) answers.
  ///
  /// It runs while the runtime is borrowed and reads nothing the runtime holds but `own`.
  fn scroll_entry(node: NodeId, own: &mut Self::Node, content: Size<f32>, offset: Point<f32>);

  /// Whether the platform draws text at sizes other than its
  /// [`FONT_SIZE`](Realization::FONT_SIZE), as it last found; see
  /// [`ResolutionSource::sizes_text`](skene_foundation::ResolutionSource::sizes_text).
  ///
  /// It runs while the runtime is borrowed and reads nothing the runtime holds.
  fn sizes_text() -> bool;

  /// Fills in what the platform's own vocabulary takes of a box's `style` once every
  /// property it declares is resolved.
  fn finish_box(style: &mut BoxStyle<Self::Scalar, Plain, Self::StyleExt>);

  /// Fills in what the platform's own vocabulary takes of a run's `style` once every
  /// property it declares is resolved, reading the tokens in force at the run through
  /// `resolver`.
  fn finish_run(
    style: &mut RunStyle<Plain, Self::StyleExt>,
    resolver: &Resolver<'_, Self::Scalar, Self::StyleExt>,
  );

  /// How far the paint of a box styled `style` reaches past its border box on each edge,
  /// in units of the layout.
  fn reach(style: &PlainBox<Self>) -> Inset<f32>;

  /// The engine the platform's text lays out through on this thread.
  fn engine() -> Self::Engine;

  /// The paragraph style every text of the platform lays out in.
  fn paragraph_style() -> ParagraphStyle;

  /// The style the run styled `run` lays its text out in; `None` is the base a line
  /// holding no run lays out in.
  fn text_style(run: Option<&PlainRun<Self>>) -> TextStyle;
}

/// The layout tree a runtime of the platform `P` keeps.
type LayoutTree<P> = Tree<Measured<<P as Drawn>::Engine>>;

/// What a platform Skene draws whole keeps of its scene.
///
/// A [`Record`] per node, the root filling the window, the overlays and text entries in
/// build order, what moved since the last restyle, the layout tree it writes from them,
/// the index of window rectangles it builds from that tree ([`Frames`]), and what the
/// next frame lays out and repaints, its damage as nodes and rectangles, beside what it
/// keeps of its window, the app's choice of scheme, the frame clock its motion samples
/// against and the text generation its flows are made under.
#[derive(Debug)]
#[must_use]
#[expect(
  clippy::struct_excessive_bools,
  reason = "each flag is an independent fact about the next frame"
)]
pub struct Runtime<P: Drawn> {
  records: NodeMap<Record<P>>,
  root: NodeId,
  /// Every overlay, in build order; later entries stack on top.
  pub overlays: Vec<NodeId>,
  /// Every text entry, in build order.
  pub entries: Vec<NodeId>,
  /// What moved since the last restyle: the resolution generation, moving when
  /// definitions, the environment or a control's state change, and the nodes the next
  /// restyle revisits.
  pub restyle: Restyle,
  /// Whether anything the layout reads changed since it was last computed.
  pub layout_dirty: bool,
  /// Whether the whole frame repaints; a change confined to a node damages it instead.
  pub paint_dirty: bool,
  /// Whether a node's cover on the window may have moved since the platform last
  /// painted: the layout, a scroll offset, a box's reach or visibility, or an overlay's
  /// opening or placement changed.
  pub covers_moved: bool,
  /// Whether a tween or an animation was still running at the last presentation, so
  /// the next frame is due at the frame rate.
  pub motion_active: bool,
  style_resolutions: u64,
  style_writes: u64,
  layout_generation: u64,
  text_generation: u64,
  tree: LayoutTree<P>,
  damaged: Vec<NodeId>,
  damaged_rects: Vec<Rect<f32>>,
  clock: Clock,
  window: Window<P>,
  choice: Option<ColorScheme>,
  reveal: Option<NodeId>,
  frames: OnceCell<Frames>,
  spare_frames: RefCell<Frames>,
}

impl<P: Drawn> Runtime<P> {
  /// A runtime whose root box, filling the window, is the scene's node `root`: a
  /// decorated vertical flex box, its frame yet to be laid out and painted, in a window
  /// whose platform reports `reported` and whose pictures paint where `painting` holds,
  /// the app having chosen no scheme, its frame clock stepped by hand from zero.
  pub fn new(root: NodeId, reported: Environment<P::Scalar>, painting: bool) -> Self {
    let root_facts = BoxFacts {
      layout: Layout::Flex {
        axis: Axis::Vertical,
        reversed: false,
      },
      container: None,
      transition_name: None,
      link: false,
      acting: false,
      decorated: true,
    };
    let mut runtime = Self {
      records: NodeMap::new(),
      root,
      overlays: Vec::new(),
      entries: Vec::new(),
      restyle: Restyle::new(),
      layout_dirty: true,
      paint_dirty: true,
      covers_moved: true,
      motion_active: false,
      style_resolutions: 0,
      style_writes: 0,
      layout_generation: 0,
      text_generation: 0,
      tree: Tree::new(P::ROUNDED),
      damaged: Vec::new(),
      damaged_rects: Vec::new(),
      clock: Clock::Manual(0.),
      window: Window::new(reported, painting),
      choice: None,
      reveal: None,
      frames: OnceCell::new(),
      spare_frames: RefCell::new(Frames::default()),
    };

    runtime.insert(
      root,
      Record::new(
        Kind::Box(BoxRecord::new(None, None, root_facts)),
        P::Node::default(),
      ),
    );

    runtime
  }

  /// Runs `operation` on the platform's runtime.
  ///
  /// # Panics
  ///
  /// Panics if no runtime is installed for this thread, or if the platform's state is
  /// borrowed already.
  pub fn with<R>(operation: impl FnOnce(&mut Self) -> R) -> R {
    P::with_runtime(operation).expect("the drawn platform to be installed for this thread")
  }

  /// Runs `operation` on the platform's runtime, `None` where none is installed for
  /// this thread.
  ///
  /// # Panics
  ///
  /// Panics if the platform's state is borrowed already.
  #[inline(always)]
  pub fn try_with<R>(operation: impl FnOnce(&mut Self) -> R) -> Option<R> {
    P::with_runtime(operation)
  }

  /// The root box, filling the window.
  #[inline(always)]
  pub const fn root(&self) -> NodeId {
    self.root
  }

  /// Stores `record` as the record of `node`, beside a fresh node of the layout tree: a
  /// container laid out by an engine of its own where a view taking decoration created
  /// its box, and laid out by its nearest such ancestor's engine where it is a
  /// layout-only box or a run; its structure moved.
  fn insert(&mut self, node: NodeId, record: Record<P>) {
    self.restyle.move_structure();
    self.invalidate_frames();
    self.tree.insert(node, record.kind.is_decorated());

    let _replaced = self.records.insert(node, record);
  }

  /// Whether `node` holds a record: it is live.
  #[inline(always)]
  #[must_use]
  pub fn contains(&self, node: NodeId) -> bool {
    self.records.contains(node)
  }

  /// The record of `node`, if it is live.
  #[inline(always)]
  #[must_use]
  pub fn record(&self, node: NodeId) -> Option<&Record<P>> {
    self.records.get(node)
  }

  /// The record of `node`, mutably, if it is live.
  #[inline(always)]
  pub fn record_mut(&mut self, node: NodeId) -> Option<&mut Record<P>> {
    self.records.get_mut(node)
  }

  /// Every live node, in index order.
  #[must_use]
  pub fn handles(&self) -> Vec<NodeId> {
    self.records.iter().map(|(node, _)| node).collect()
  }

  /// Every live node with its record, in index order.
  pub fn records(&self) -> impl Iterator<Item = (NodeId, &Record<P>)> {
    self.records.iter()
  }

  /// Every live node with its record, mutably, in index order.
  pub fn records_mut(&mut self) -> impl Iterator<Item = (NodeId, &mut Record<P>)> {
    self.records.iter_mut()
  }

  /// How many nodes the runtime holds a record of.
  #[must_use]
  pub fn record_count(&self) -> usize {
    self.records.values().count()
  }

  /// Releases the record of `node`, retiring it from the overlays, the entries, the
  /// restyle, the layout tree, the reveal queued and the index of window rectangles, its
  /// structure moved, what it covered at the last paint damaged and the frame
  /// repainting; the record removed, for the platform to clear its own references, or
  /// `None` where `node` held none.
  pub fn destroy(&mut self, node: NodeId) -> Option<Record<P>> {
    let record = self.records.remove(node)?;

    self.damaged_rects.extend(record.painted);

    self.tree.remove(node);
    self.overlays.retain(|overlay| *overlay != node);
    self.entries.retain(|entry| *entry != node);

    if self.reveal == Some(node) {
      self.reveal = None;
    }

    self.restyle.read_interaction(node, false);
    self.restyle.move_structure();
    self.invalidate_frames();
    self.paint_dirty = true;

    Some(record)
  }

  /// The frame clock's reading, in seconds.
  #[inline(always)]
  #[must_use]
  pub fn now(&self) -> f64 {
    self.clock.now()
  }

  /// Runs the frame clock as `clock`.
  #[inline(always)]
  pub const fn set_clock(&mut self, clock: Clock) {
    self.clock = clock;
  }

  /// Steps the frame clock forward by `seconds`, stepping it by hand from then on.
  #[inline(always)]
  pub fn advance_clock(&mut self, seconds: f64) {
    self.clock.advance(seconds);
  }

  /// The text generation every flow is made under: moved where the platform's engine
  /// came to lay the same text out differently.
  #[inline(always)]
  #[must_use]
  pub const fn text_generation(&self) -> u64 {
    self.text_generation
  }

  /// Measures every node's text again, after the platform's engine came to lay the same
  /// text out differently: every node revisited at the next restyle and measured again
  /// at the next layout, and the text generation moved, so each flow made under the last
  /// one is unlike the one made afresh.
  pub fn remeasure(&mut self) {
    self.restyle.revisit_every();
    self.text_generation = self.text_generation.wrapping_add(1);

    for (node, _) in self.records.iter() {
      self.tree.mark_dirty(node);
    }
  }

  /// The layout tree: a node per record, its taffy style and its leaf as the last
  /// [`write_styles`](Self::write_styles) wrote them, its children as the last
  /// [`write_children`](Self::write_children) did, laid out by the last
  /// [`compute`](Self::compute).
  #[inline(always)]
  #[must_use]
  pub const fn tree(&self) -> &Tree<Measured<P::Engine>> {
    &self.tree
  }

  /// How many node styles have been resolved from their declarations.
  #[inline(always)]
  #[must_use]
  pub const fn style_resolutions(&self) -> u64 {
    self.style_resolutions
  }

  /// How many taffy styles have been written into the layout tree.
  #[inline(always)]
  #[must_use]
  pub const fn style_writes(&self) -> u64 {
    self.style_writes
  }

  /// How many layouts have been computed.
  #[inline(always)]
  #[must_use]
  pub const fn layout_generation(&self) -> u64 {
    self.layout_generation
  }

  /// An empty flow laid out through the platform's engine and paragraph style, a line
  /// holding no run laid out in its base text style, in whole units where its layout is
  /// rounded, under the runtime's text generation.
  pub fn flow(&self) -> Flow<P::Engine> {
    Flow::new(
      P::engine(),
      P::text_style(None),
      P::paragraph_style(),
      P::ROUNDED,
      self.text_generation,
    )
  }
}
