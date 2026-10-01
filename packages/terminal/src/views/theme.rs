//! The one theme view: a scope defining a token set beneath it.

use skene_foundation::{AnyKind, ChildBuilder, NoChild, NodeId, TokenSet, View};

use crate::{ActivePlatform, TokenValue};

/// Defines the token set `S` for everything beneath it.
///
/// Realizes one layout-invisible scope node, provides `Memo<S>` through context so
/// [`ThemedIn::current`](crate::ThemedIn::current) and [`ThemedIn::watch`](crate::ThemedIn::watch)
/// read it, and rewrites the scope's definitions when the palette changes: no consumer is touched
/// and nothing rebuilds. Nesting a theme redefines beneath it. Holds [`NoChild`] until
/// [`set_child_builder`](Self::set_child_builder) supplies the child; the child is a
/// builder closure because it is built under the owner the context is provided on.
#[must_use]
pub struct Theme<S: TokenSet<TokenValue>, C = NoChild>(
  skene_foundation::Theme<ActivePlatform, S, C>,
);

impl<S: TokenSet<TokenValue>, C> std::fmt::Debug for Theme<S, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(formatter)
  }
}

impl<S: TokenSet<TokenValue>> Theme<S> {
  /// A theme defining `palette`.
  #[inline(always)]
  pub fn new(palette: S) -> Self {
    Self(skene_foundation::Theme::new(palette))
  }

  /// A theme defining the palette `palette` returns, recomputed whenever a signal it
  /// reads changes.
  #[inline(always)]
  pub fn derive(palette: impl Fn() -> S + 'static) -> Self {
    Self(skene_foundation::Theme::derive(palette))
  }

  /// Sets the child, built by `child` under the theme.
  #[inline(always)]
  pub fn set_child_builder<V: View<ActivePlatform>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Theme<S, ChildBuilder<ActivePlatform>> {
    Theme(self.0.set_child_builder(child))
  }
}

impl<S: TokenSet<TokenValue>, C> AnyKind for Theme<S, C> {}

impl<S: TokenSet<TokenValue>> View<ActivePlatform> for Theme<S, ChildBuilder<ActivePlatform>> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
