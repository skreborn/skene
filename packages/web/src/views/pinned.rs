//! The wrapper pinning a stack child to the stack's edges.

use skene_foundation::{AnyView, IntoChildOf, NoChild, PinnedBase, StackChild, View, Wraps};

use crate::Active;

/// Pins the child it wraps against the edges of a [`Stack`](crate::Stack), out of flow.
///
/// Creates no node: the edges become the `pinned` property of the child's own style, so
/// they compile, condition and derive like any property. An edge left `Auto` puts the
/// child where the stack's alignment does on that axis; every edge is `Auto` until
/// [`set_edges`](Self::set_edges). Accepted only where a stack's children are; holds
/// [`NoChild`] until `set_child` supplies the child.
///
/// ```
/// use skene_web::prelude::*;
///
/// let corner = Stack::new().set_children((
///   Text::new("Content"),
///   Pinned::new()
///     .set_edges(Inset {
///       start: Dimension::Auto,
///       top: Dimension::Length(0.),
///       end: Dimension::Length(0.),
///       bottom: Dimension::Auto,
///     })
///     .set_child(Text::new("New")),
/// ));
/// ```
#[derive(Debug, Wraps)]
#[must_use]
pub struct Pinned<C = NoChild>(skene_foundation::Pinned<Active, C>);

impl Default for Pinned {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl PinnedBase<Active> for Pinned {}

impl Pinned {
  /// Sets the child.
  #[inline(always)]
  pub fn set_child(self, child: impl View<Active>) -> Pinned<AnyView<Active>> {
    Pinned(self.0.set_child(child))
  }
}

impl IntoChildOf<Active, StackChild> for Pinned<AnyView<Active>> {
  #[inline(always)]
  fn into_child(self) -> AnyView<Active> {
    self.0.into_child()
  }
}
