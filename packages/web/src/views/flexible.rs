//! The wrapper giving a flex child its flexibility.

use skene_foundation::{AnyView, FlexChild, FlexibleBase, IntoChildOf, NoChild, View, Wraps};

use crate::ActivePlatform;

/// Gives the child it wraps its flexibility in a [`Row`](crate::Row) or a
/// [`Column`](crate::Column).
///
/// Creates no node: the flexibility becomes the `flex` property of the child's own
/// style, and the alignment its `self_alignment`, so they compile, condition and derive
/// like any property. Accepted only where a flex container's children are; holds
/// [`NoChild`] until `set_child` supplies the child.
///
/// ```
/// use skene_web::prelude::*;
///
/// let bar = Row::new().set_children((
///   Flexible::new()
///     .set_flexibility(Flexibility::grow(1.))
///     .set_child(Text::new("Grows")),
///   Text::new("Stays"),
/// ));
/// ```
#[derive(Debug, Wraps)]
#[must_use]
pub struct Flexible<C = NoChild>(skene_foundation::Flexible<ActivePlatform, C>);

impl Default for Flexible {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl FlexibleBase<ActivePlatform> for Flexible {}

impl Flexible {
  /// Sets the child.
  #[inline(always)]
  pub fn set_child(self, child: impl View<ActivePlatform>) -> Flexible<AnyView<ActivePlatform>> {
    Flexible(self.0.set_child(child))
  }
}

impl IntoChildOf<ActivePlatform, FlexChild> for Flexible<AnyView<ActivePlatform>> {
  #[inline(always)]
  fn into_child(self) -> AnyView<ActivePlatform> {
    self.0.into_child()
  }
}
