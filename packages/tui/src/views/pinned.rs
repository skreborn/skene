//! The wrapper pinning a stack child to the stack's edges.

use skene_foundation::{AnyView, IntoChildOf, PinnedBase, StackChild, Wraps};

use crate::Active;

/// Pins the child it wraps against the edges of a [`Stack`](crate::Stack), out of flow.
///
/// Creates no node: the edges become the `pinned` property of the child's own style, so
/// they compile, condition and derive like any property. An edge left `Auto` puts the
/// child where the stack's alignment does on that axis; every edge is `Auto` until
/// [`set_edges`](Self::set_edges). Accepted only where a stack's children are; a wrapper
/// without its child renders nothing and takes no room.
///
/// ```
/// use skene_tui::prelude::*;
///
/// let corner = Stack::new().set_children((
///   Text::new("Content"),
///   Pinned::new()
///     .set_edges(Inset {
///       start: Dimension::Auto,
///       top: Dimension::Length(0),
///       end: Dimension::Length(0),
///       bottom: Dimension::Auto,
///     })
///     .set_child(Text::new("New")),
/// ));
/// ```
#[derive(Debug, Wraps)]
#[must_use]
pub struct Pinned(skene_foundation::Pinned<Active>);

impl Default for Pinned {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl PinnedBase<Active> for Pinned {}

impl IntoChildOf<Active, StackChild> for Pinned {
  #[inline(always)]
  fn into_child(self) -> AnyView<Active> {
    self.0.into_child()
  }
}
