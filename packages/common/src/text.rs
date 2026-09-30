//! A text run on its own line.

use skene_core::{AnyKind, NodeId, Platform, RunDisplay, View};

use crate::{realize::Source, run::Run};

/// A run of text on its own line.
///
/// A property left unset is inherited through its [`typography`](crate::typography)
/// token; the medium's own text properties come through the platform's style extension.
#[must_use]
pub struct Text<P: Platform> {
  content: Source<String>,
  run: Run<P>,
}

impl<P: Platform> std::fmt::Debug for Text<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Text").finish_non_exhaustive()
  }
}

impl<P: Platform> Text<P> {
  /// A run reading `content`.
  #[inline(always)]
  pub fn new(content: impl Into<String>) -> Self {
    Self {
      content: Source::Fixed(content.into()),
      run: Run::new(),
    }
  }

  /// A run whose text is recomputed whenever a signal `content` reads changes.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self {
      content: Source::derive(content),
      run: Run::new(),
    }
  }

  crate::run_setters! { run }
}

impl<P: Platform> AnyKind for Text<P> {}

impl<P: Platform> View<P> for Text<P> {
  fn build(self) -> NodeId {
    let node = self.run.build(RunDisplay::Block);

    self
      .content
      .bind(node, |node, text| P::set_text(node, text));

    node
  }
}
