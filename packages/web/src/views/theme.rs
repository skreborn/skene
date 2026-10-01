//! The one theme view: a scope defining a token set beneath it.

use skene_foundation::{NoChild, TokenSet};

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

skene_foundation::impl_theme!(Theme, ActivePlatform);
