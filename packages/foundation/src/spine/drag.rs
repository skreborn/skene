//! Drag and drop over the scene: what a drag carries, where it started and the node
//! taking it, which every platform moves through one implementation.

use std::{any::Any, fmt, rc::Rc};

use crate::spine::{NodeId, Realization, Scene, task::run_handler};

/// What a drag carries: a value of the app's, and the text another app may read.
#[derive(Clone, Default)]
#[must_use]
pub struct DragData {
  value: Option<Rc<dyn Any>>,
  text: Option<String>,
}

impl fmt::Debug for DragData {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("DragData")
      .field("text", &self.text)
      .finish_non_exhaustive()
  }
}

impl DragData {
  /// A drag carrying `value`.
  pub fn new(value: impl Any) -> Self {
    Self {
      value: Some(Rc::new(value)),
      text: None,
    }
  }

  /// The drag with `text` beside its value, which another app may read.
  pub fn with_text(mut self, text: impl Into<String>) -> Self {
    self.text = Some(text.into());

    self
  }

  /// The value the drag carries, when it is a `T`.
  #[must_use]
  pub fn value<T: 'static>(&self) -> Option<&T> {
    self.value.as_deref()?.downcast_ref()
  }

  /// The text the drag carries, if any.
  #[inline(always)]
  #[must_use]
  pub fn text(&self) -> Option<&str> {
    self.text.as_deref()
  }
}

/// What a node taking drops is asked of a drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum DropPhase {
  /// Whether it would take the drag, which is over it; it acts on nothing.
  Offered,
  /// To take the drag, dropped on it.
  Dropped,
}

/// What a drag from a node carries, built as the drag starts.
pub type DragSource = dyn Fn() -> DragData;

/// A node's answer to a drag in `DropPhase`: whether it takes the drag.
pub type DropHandler = dyn Fn(&DragData, DropPhase) -> bool;

/// Starts a drag from `node`: the nearest node at or above it offering drag data builds
/// what the drag carries, and holds `Dragged` until the drag ends.
///
/// That node and the data, or `None` where no node offers any.
#[must_use]
pub fn start_drag<P: Realization>(node: NodeId) -> Option<(NodeId, DragData)> {
  let (source, build) = Scene::<P>::with(|scene| scene.drag_source(node))?;
  let data = run_handler(build.owner(), &*build);

  Scene::<P>::with(|scene| scene.begin_drag(source, data.clone()));

  Some((source, data))
}

/// Moves the drag in progress over `node`: the nearest node at or above it that takes
/// what the drag carries becomes its target, holding `Targeted`.
///
/// The target, `None` where no node takes the drag or none is in progress.
#[must_use]
pub fn move_drag<P: Realization>(node: Option<NodeId>) -> Option<NodeId> {
  let (data, candidates) = Scene::<P>::with(|scene| {
    let data = scene.drag_data()?.clone();
    let candidates = node.map_or_else(Vec::new, |node| scene.drop_candidates(node));

    Some((data, candidates))
  })?;
  let target = candidates.into_iter().find_map(|(node, handlers)| {
    handlers
      .iter()
      .any(|handler| run_handler(handler.owner(), || handler(&data, DropPhase::Offered)))
      .then_some(node)
  });

  Scene::<P>::with(|scene| scene.set_drag_target(target));

  target
}

/// Drops the drag in progress on its target and ends it; whether the target took it.
#[must_use]
pub fn drop_drag<P: Realization>() -> bool {
  let Some((data, target)) = Scene::<P>::with(|scene| {
    let data = scene.drag_data()?.clone();
    let target = scene.drag_target()?;

    Some((data, scene.drop_handlers(target)))
  })
  else {
    end_drag::<P>();

    return false;
  };
  let taken = target
    .iter()
    .any(|handler| run_handler(handler.owner(), || handler(&data, DropPhase::Dropped)));

  end_drag::<P>();

  taken
}

/// Ends the drag in progress, dropped or not: its source holds `Dragged` and its
/// target `Targeted` no longer.
pub fn end_drag<P: Realization>() {
  Scene::<P>::with(Scene::finish_drag);
}
