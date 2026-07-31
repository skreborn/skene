//! The batched styling helper for web views.
//!
//! A web view—built-in or library-authored—folds its static properties into one
//! content-addressed class ([`set_style`](super::set_style)) and wires each reactive
//! property to a single-property inline override ([`override_style`](super::override_style))
//! that wins over the class on change. [`Styler`] drives that split so a raw web view gets
//! the same batched class-interning path the built-ins do, rather than rewriting the shared
//! stylesheet as a property animates.

use skene_core::{NodeId, Source};

use crate::{Declarations, override_style, set_style};

/// Realizes a node's style from static and reactive property sources.
///
/// Static sources fold into one interned class applied on [`finish`](Self::finish); each
/// derived source additionally pushes a single-property inline override whenever it
/// changes. Seed the initial [`Declarations`] with any always-present rules (layout,
/// display), add each property with [`prop`](Self::prop), then [`finish`](Self::finish).
#[must_use]
pub struct Styler {
  node: NodeId,
  initial: Declarations,
}

impl Styler {
  /// Starts styling `node` from `initial`, the declarations always present regardless of
  /// the view's properties.
  #[inline(always)]
  pub fn new(node: NodeId, initial: Declarations) -> Self {
    Self { node, initial }
  }

  /// Adds one property from `source`, applying `apply` to fold its value into the
  /// declarations. A `None` source contributes nothing. A static source folds into the
  /// interned class; a derived source instead binds an inline override that re-applies on
  /// every change, so the shared class stays free of transient values.
  #[inline(always)]
  pub fn prop<T: 'static>(
    mut self,
    source: Option<Source<T>>,
    apply: impl Fn(Declarations, T) -> Declarations + 'static,
  ) -> Self {
    let Some(source) = source
    else {
      return self;
    };

    match source {
      Source::Static(value) => self.initial = apply(self.initial, value),
      source @ Source::Derived(_) => {
        let node = self.node;

        source.bind(move |value| {
          let delta = apply(Declarations::new(), value);

          override_style(node, &delta);
        });
      }
    }

    self
  }

  /// Interns the accumulated static declarations as the node's class, or assigns none when
  /// there are no static declarations (an unstyled inline run needs no class).
  #[inline(always)]
  pub fn finish(self) {
    if self.initial.is_empty() {
      return;
    }

    set_style(self.node, &self.initial);
  }
}
