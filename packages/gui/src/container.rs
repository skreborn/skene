//! Decoration and sizing around exactly one child.

use skene_core::{IntoChildOf, NodeId, PlainChild, View};

use crate::{HasChildren, StyledBox};

/// Decoration and sizing around exactly one child.
///
/// The plain box of the view vocabulary: no layout of its own—the child sits in the
/// box's content area. Several children take a [`Row`](crate::Row),
/// [`Column`](crate::Column), or another layout container.
#[must_use]
pub struct Container {
  base: StyledBox,
}

impl Container {
  /// Creates an empty container.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: StyledBox::new(),
    }
  }

  /// Sets the child.
  #[inline(always)]
  pub fn set_child(mut self, child: impl IntoChildOf<PlainChild>) -> Self {
    self.base = self.base.set_child(child);

    self
  }
}

impl Default for Container {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl crate::HasSize for Container {
  #[inline(always)]
  fn size_box(&mut self) -> &mut StyledBox {
    &mut self.base
  }
}

impl crate::HasDecoration for Container {
  #[inline(always)]
  fn decoration_box(&mut self) -> &mut StyledBox {
    &mut self.base
  }
}

impl crate::HasEvents for Container {
  #[inline(always)]
  fn events_box(&mut self) -> &mut StyledBox {
    &mut self.base
  }
}

impl View for Container {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.base.build()
  }
}
