//! The one theme view: a scope defining a token set beneath it.

use skene_foundation::{ThemeBase, TokenSet, View, Wraps};

use crate::{Active, TokenValue};

/// Defines the token set `S` for everything beneath it.
///
/// Realizes one layout-invisible scope node, provides `Memo<S>` through context so
/// [`ThemedIn::current`](crate::ThemedIn::current) and
/// [`ThemedIn::watch`](crate::ThemedIn::watch) read it, and rewrites the scope's
/// definitions when the palette changes: no consumer is touched and nothing rebuilds.
/// Nesting a theme redefines beneath it. The child is a builder closure
/// [`set_child_builder`](Self::set_child_builder) supplies, built under the owner the
/// context is provided on; without it the theme realizes no scope and renders nothing.
///
/// ```
/// use skene_terminal::prelude::*;
///
/// let themed = Theme::new(Typography::default().set_size(2.))
///   .set_child_builder(|| Text::new("Set twice the cell"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Theme<S: TokenSet<TokenValue>>(skene_foundation::Theme<Active, S>);

impl<S: TokenSet<TokenValue>> ThemeBase<Active, S> for Theme<S> {}
