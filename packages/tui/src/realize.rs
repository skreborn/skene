//! The shared static-versus-derived property split every realization performs.

use skene_core::{NodeId, Source};

/// Reactive-property bindings collected during a build, run once the node exists.
pub(crate) type Binders = Vec<Box<dyn FnOnce(NodeId)>>;

/// Splits one property: a static source fills `slot`, a derived one queues a binder that
/// re-applies through `update` on every change.
pub(crate) fn split_prop<T: Clone + 'static, P: 'static>(
  source: Option<Source<T>>,
  slot: &mut Option<T>,
  binders: &mut Binders,
  to_prop: impl Fn(T) -> P + 'static,
  update: fn(NodeId, P),
) {
  match source {
    None => {}
    Some(Source::Static(value)) => *slot = Some(value),
    Some(source) => binders.push(Box::new(move |node| {
      source.bind(move |value| update(node, to_prop(value)));
    })),
  }
}
