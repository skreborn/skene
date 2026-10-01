//! A text run on its own line.

use skene_foundation::{AnyKind, NodeId, View};

use crate::ActivePlatform;

/// A run of text on its own line.
///
/// A property left unset is inherited through its [`typography`](crate::typography)
/// token.
#[derive(Debug)]
#[must_use]
pub struct Text(skene_foundation::Text<ActivePlatform>);

impl Text {
  /// A run reading `content`.
  #[inline(always)]
  pub fn new(content: impl Into<String>) -> Self {
    Self(skene_foundation::Text::new(content))
  }

  /// A run whose text is recomputed whenever a signal `content` reads changes.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self(skene_foundation::Text::derive(content))
  }

  skene_foundation::run_surface! { 0, ActivePlatform }
}

impl AnyKind for Text {}

impl View<ActivePlatform> for Text {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
