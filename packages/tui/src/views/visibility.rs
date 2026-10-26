//! A child shown or hidden by a condition.

use skene_foundation::{View, VisibilityBase, Wraps};

use crate::Active;

/// Shows or hides one child, preserving its state.
///
/// The child is built the first time it shows, then attached or detached between a
/// pair of anchors when the condition changes, and left where it stands while it
/// holds. While hidden its effects pause, catching up once it shows again.
///
/// Without the child [`set_child`](Self::set_child) supplies, it renders nothing.
///
/// ```
/// use skene_tui::prelude::*;
///
/// let open = Signal::new(false);
/// let details = Visibility::new()
///   .derive_shown(move || open.get())
///   .set_child(Text::new("The details"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Visibility(skene_foundation::Visibility<Active>);

impl Default for Visibility {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl VisibilityBase<Active> for Visibility {}
