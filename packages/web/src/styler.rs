use std::rc::Rc;

use skene_core::{NodeId, Source};
use skene_reactive::Effect;

/// Realizes a node's style from a base value and reactive property sources.
///
/// Every static source folds into one initial style, applied once by
/// [`Styler::finish`]. Each derived source is wired to push a single-property
/// delta through `commit` when it changes.
#[must_use = "call `finish` to apply the style"]
pub struct Styler<S> {
  node: NodeId,
  initial: S,
  commit: Rc<dyn Fn(NodeId, S)>,
}

impl<S: Default + 'static> Styler<S> {
  /// Starts styling `node` from `base`, applying each style value with `commit`.
  #[inline(always)]
  pub fn new(node: NodeId, base: S, commit: impl Fn(NodeId, S) + 'static) -> Self {
    Styler {
      node,
      initial: base,
      commit: Rc::new(commit),
    }
  }

  /// Adds one property. `apply` sets it on a style value; a static `source` folds
  /// into the initial style, a derived one pushes a single-property delta on change.
  #[inline(always)]
  pub fn prop<T: Clone + 'static>(
    mut self,
    source: Option<Source<T>>,
    apply: impl Fn(S, T) -> S + 'static,
  ) -> Self {
    let Some(source) = source
    else {
      return self;
    };

    self.initial = apply(self.initial, source.get());

    if source.is_derived() {
      let node = self.node;
      let commit = Rc::clone(&self.commit);

      Effect::new(move |previous: Option<()>| {
        let value = source.get();

        if previous.is_some() {
          commit(node, apply(S::default(), value));
        }
      });
    }

    self
  }

  /// Applies the accumulated initial style.
  #[inline(always)]
  pub fn finish(self) {
    (self.commit)(self.node, self.initial);
  }
}
