//! What every view's build shares: reaching a node's sink, and binding the render
//! effect behind each derived slot.

use skene_core::next::{BoxSlot, Driver, NodeId, RunSlot, Scene};
use skene_reactive::Effect;

/// Runs `operation` on the sink of `node`, if the node is live.
#[inline(always)]
pub fn with_sink<D: Driver, R>(node: NodeId, operation: impl FnOnce(&D::Sink) -> R) -> Option<R> {
  Scene::<D>::with(|scene| scene.record(node).map(|record| operation(record.sink())))
}

/// Binds a render effect that computes `slot`'s leaf and writes it into the box
/// `node`, now and whenever a signal the leaf reads changes.
pub fn bind_box_slot<D: Driver>(node: NodeId, slot: BoxSlot<D::Scalar, D::StyleExt>) {
  let BoxSlot { slot, compute, .. } = slot;

  Effect::render(move |_: Option<()>| {
    let value = compute();

    with_sink::<D, _>(node, |sink| D::default().update_box(sink, slot, &value));
  });
}

/// Binds a render effect that computes `slot`'s leaf and writes it into the run
/// `node`, now and whenever a signal the leaf reads changes.
pub fn bind_run_slot<D: Driver>(node: NodeId, slot: RunSlot<D::StyleExt>) {
  let RunSlot { slot, compute, .. } = slot;

  Effect::render(move |_: Option<()>| {
    let value = compute();

    with_sink::<D, _>(node, |sink| D::default().update_run(sink, slot, &value));
  });
}
