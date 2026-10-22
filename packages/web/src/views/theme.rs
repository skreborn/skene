//! The one theme view: a scope defining a token set beneath it.

use skene_foundation::{ChildBuilder, NoChild, ThemeBase, TokenSet, View, Wraps};

use crate::{Active, TokenValue};

/// Defines the token set `S` for everything beneath it.
///
/// Realizes one layout-invisible scope node, provides `Memo<S>` through context so
/// [`ThemedIn::current`](crate::ThemedIn::current) and
/// [`ThemedIn::watch`](crate::ThemedIn::watch) read it, and rewrites the scope's
/// definitions when the palette changes: no consumer is touched and nothing rebuilds.
/// Nesting a theme redefines beneath it. Holds [`NoChild`] until
/// [`set_child_builder`](Self::set_child_builder) supplies the child as a builder
/// closure, built under the owner the context is provided on.
///
/// ```
/// use skene_web::prelude::*;
///
/// let themed = Theme::new(Typography::default().set_size(18.))
///   .set_child_builder(|| Text::new("Set in eighteen pixels"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Theme<S: TokenSet<TokenValue>, C = NoChild>(skene_foundation::Theme<Active, S, C>);

impl<S: TokenSet<TokenValue>> ThemeBase<Active, S> for Theme<S> {}

impl<S: TokenSet<TokenValue>> Theme<S> {
  /// Sets the child, built by `child` under the theme.
  #[inline(always)]
  pub fn set_child_builder<V: View<Active>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Theme<S, ChildBuilder<Active>> {
    Theme(self.0.set_child_builder(child))
  }
}
