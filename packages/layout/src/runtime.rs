//! The drawn runtime: what every platform Skene draws whole runs beneath its realization.
//!
//! A platform Skene draws whole ([`Drawn`]) keeps one [`Runtime`] in its own state: a
//! [`Record`] per node of the scene, its kind with the facts its creation carried, its
//! kept and resolved style and its motion, beside the root filling the window, the
//! overlays and text entries in build order, what moved since the last restyle and what
//! the next frame lays out and repaints. The runtime reads the scene's structure as the
//! layout and the display see it ([`Runtime::layout_children`], [`Runtime::displayed`]),
//! and each node's style is presented on the platform's frame clock, its transitions and
//! keyframe animations laid over it ([`BoxMotion`], [`RunMotion`]).
//!
//! A platform keeps its own per-node state in a side table of its own keyed by the node,
//! never in the runtime's records.

mod motion;
mod record;
mod structure;

use skene_foundation::{Axis, BoxFacts, Layout, NodeId, NodeMap, Realization, TokenValue};

pub use self::{
  motion::{BoxMotion, Presented, RunMotion, Sample},
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
};
use crate::Restyle;

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
pub trait Drawn: Realization<TokenValue = TokenValue<<Self as Realization>::Scalar>> {
  /// Runs `operation` on the platform's runtime, `None` where none is installed for
  /// this thread.
  ///
  /// The platform's state is borrowed while `operation` runs, and a call nested inside
  /// it panics, so `operation` runs nothing of the app's: no handler, and no write whose
  /// effects run before it returns. The platform runs its handlers with nothing
  /// borrowed, so a handler reaches the runtime as any other caller does.
  fn with_runtime<R>(operation: impl FnOnce(&mut Runtime<Self>) -> R) -> Option<R>;
}

/// What a platform Skene draws whole keeps of its scene.
///
/// A [`Record`] per node, the root filling the window, the overlays and text entries in
/// build order, what moved since the last restyle, and what the next frame lays out and
/// repaints.
#[derive(Debug)]
#[must_use]
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
  damaged: Vec<NodeId>,
}

impl<P: Drawn> Runtime<P> {
  /// A runtime whose root box, filling the window, is the scene's node `root`: a
  /// decorated vertical flex box, its frame yet to be laid out and painted.
  pub fn new(root: NodeId) -> Self {
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
      damaged: Vec::new(),
    };

    runtime.insert(root, Kind::Box(BoxRecord::new(None, None, root_facts)));

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

  /// Stores a record of `kind` as the record of `node`, its structure moved.
  pub fn insert(&mut self, node: NodeId, kind: Kind<P>) {
    self.restyle.move_structure();

    let _replaced = self.records.insert(node, Record::new(node, kind));
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

  /// How many nodes the runtime holds a record of.
  #[must_use]
  pub fn record_count(&self) -> usize {
    self.records.values().count()
  }

  /// Releases the record of `node`, retiring it from the overlays, the entries and the
  /// restyle, its structure moved and the frame repainting; the record removed, for the
  /// platform to clear its own references, or `None` where `node` held none.
  pub fn destroy(&mut self, node: NodeId) -> Option<Record<P>> {
    let record = self.records.remove(node)?;

    self.overlays.retain(|overlay| *overlay != node);
    self.entries.retain(|entry| *entry != node);
    self.restyle.read_interaction(node, false);
    self.restyle.move_structure();
    self.paint_dirty = true;

    Some(record)
  }

  /// Marks `node` for repaint: what it covered at the last paint and what it covers now
  /// are painted again at the next frame.
  #[inline(always)]
  pub fn damage(&mut self, node: NodeId) {
    self.damaged.push(node);
  }

  /// Whether a node is marked for repaint.
  #[inline(always)]
  #[must_use]
  pub const fn has_damage(&self) -> bool {
    !self.damaged.is_empty()
  }

  /// Takes the nodes marked for repaint.
  #[inline(always)]
  pub fn take_damage(&mut self) -> Vec<NodeId> {
    std::mem::take(&mut self.damaged)
  }
}
