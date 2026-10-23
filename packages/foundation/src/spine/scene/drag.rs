//! The drag in progress: where it started, what it carries, and the node taking it.

use std::rc::Rc;

use super::Scene;
use crate::spine::{DragData, DragSource, DropHandler, Handler, NodeId, Realization, Registered};

/// A drag in progress: the node it started from, what it carries, and the node taking
/// it.
pub(super) struct DragRecord {
  pub(super) source: NodeId,
  pub(super) data: DragData,
  pub(super) target: Option<NodeId>,
}

/// A drag source, out of a handler.
fn pick_drag_source(handler: &Handler) -> Option<&Rc<DragSource>> {
  match handler {
    Handler::DragStart(run) => Some(run),
    _ => None,
  }
}

/// A drop handler, out of a handler.
fn pick_drop(handler: &Handler) -> Option<&Rc<DropHandler>> {
  match handler {
    Handler::Drop(run) => Some(run),
    _ => None,
  }
}

impl<P: Realization> Scene<P> {
  /// The nearest node at or above `node` offering drag data, with what builds it.
  #[must_use]
  pub fn drag_source(&self, node: NodeId) -> Option<(NodeId, Registered<DragSource>)> {
    self.ancestry(node).into_iter().find_map(|at| {
      let source = self.handlers(at)?.select(pick_drag_source).last()?;

      Some((at, source))
    })
  }

  /// The nodes at or above `node` taking drops, innermost first, each with its drop
  /// handlers.
  #[must_use]
  pub fn drop_candidates(&self, node: NodeId) -> Vec<(NodeId, Vec<Registered<DropHandler>>)> {
    self
      .ancestry(node)
      .into_iter()
      .filter_map(|at| {
        let handlers = self.drop_handlers(at);

        (!handlers.is_empty()).then_some((at, handlers))
      })
      .collect()
  }

  /// `node`'s drop handlers.
  #[must_use]
  pub fn drop_handlers(&self, node: NodeId) -> Vec<Registered<DropHandler>> {
    self
      .handlers(node)
      .map_or_else(Vec::new, |handlers| handlers.select(pick_drop).collect())
  }

  /// The node the drag in progress started from.
  #[inline(always)]
  #[must_use]
  pub fn dragged(&self) -> Option<NodeId> {
    self.drag.as_ref().map(|drag| drag.source)
  }

  /// What the drag in progress carries.
  #[inline(always)]
  #[must_use]
  pub fn drag_data(&self) -> Option<&DragData> {
    self.drag.as_ref().map(|drag| &drag.data)
  }

  /// The node taking the drag in progress.
  #[inline(always)]
  #[must_use]
  pub fn drag_target(&self) -> Option<NodeId> {
    self.drag.as_ref().and_then(|drag| drag.target)
  }

  /// Starts a drag from `source` carrying `data`.
  pub(in crate::spine) fn begin_drag(&mut self, source: NodeId, data: DragData) {
    self.drag = Some(DragRecord {
      source,
      data,
      target: None,
    });
    self.held.touch();
  }

  /// Makes `target` the node taking the drag in progress.
  pub(in crate::spine) fn set_drag_target(&mut self, target: Option<NodeId>) {
    if let Some(drag) = &mut self.drag
      && drag.target != target
    {
      drag.target = target;
      self.held.touch();
    }
  }

  /// Ends the drag in progress.
  pub(in crate::spine) fn finish_drag(&mut self) {
    if self.drag.take().is_some() {
      self.held.touch();
    }
  }
}
