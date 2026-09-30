//! What the last layout measured of a node, in cells.

use skene_core::Size;

use super::{Handle, Records};

impl Records {
  /// A node's laid-out size, in cells.
  pub(crate) fn node_size(&self, handle: Handle) -> (i64, i64) {
    let Some(taffy) = self.record(handle).and_then(|record| record.taffy)
    else {
      return (0, 0);
    };

    match self.taffy.layout(taffy) {
      #[expect(
        clippy::cast_possible_truncation,
        reason = "a laid-out size is rounded and counts cells far inside i64"
      )]
      Ok(layout) => (
        layout.size.width.round().max(0.) as i64,
        layout.size.height.round().max(0.) as i64,
      ),
      Err(_) => (0, 0),
    }
  }

  /// A childless box's laid-out span in eighth cells from its parent's origin: where
  /// it begins and how far it extends, unrounded, so boxes beside each other meet
  /// where the layout put them. `None` for a box with laid-out children, or one of no
  /// width.
  pub(crate) fn smooth_span(&self, handle: Handle) -> Option<(isize, isize)> {
    let record = self.record(handle)?;
    let laid_out = |handle: Handle| {
      self
        .record(handle)
        .is_some_and(|record| record.kind.is_laid_out())
    };

    if self.children(handle).into_iter().any(laid_out) {
      return None;
    }

    let layout = self.taffy.unrounded_layout(record.taffy?);
    #[expect(
      clippy::cast_possible_truncation,
      reason = "a position in eighth cells stays far inside isize"
    )]
    let start = (layout.location.x * 8.).round() as isize;
    #[expect(
      clippy::cast_possible_truncation,
      reason = "an extent in eighth cells stays far inside isize"
    )]
    let extent = (layout.size.width.max(0.) * 8.).round() as isize;

    (extent > 0).then_some((start, extent))
  }

  /// A node's laid-out position within its layout parent, in cells.
  pub(crate) fn node_location(&self, handle: Handle) -> (i64, i64) {
    let Some(taffy) = self.record(handle).and_then(|record| record.taffy)
    else {
      return (0, 0);
    };

    match self.taffy.layout(taffy) {
      #[expect(
        clippy::cast_possible_truncation,
        reason = "a laid-out position is rounded and counts cells far inside i64"
      )]
      Ok(layout) => (
        layout.location.x.round() as i64,
        layout.location.y.round() as i64,
      ),
      Err(_) => (0, 0),
    }
  }

  /// A node's border widths, in cells: left, top, right, bottom.
  pub(crate) fn node_border(&self, handle: Handle) -> (i64, i64, i64, i64) {
    let Some(taffy) = self.record(handle).and_then(|record| record.taffy)
    else {
      return (0, 0, 0, 0);
    };

    match self.taffy.layout(taffy) {
      #[expect(
        clippy::cast_possible_truncation,
        reason = "a border is rounded and counts cells far inside i64"
      )]
      Ok(layout) => (
        layout.border.left.round() as i64,
        layout.border.top.round() as i64,
        layout.border.right.round() as i64,
        layout.border.bottom.round() as i64,
      ),
      Err(_) => (0, 0, 0, 0),
    }
  }

  /// A node's size inside its border, in cells: what its size observers hear.
  pub(crate) fn client_size(&self, handle: Handle) -> Size<u32> {
    let (width, height) = self.node_size(handle);
    let (left, top, right, bottom) = self.node_border(handle);

    Size::new(
      u32::try_from(width - left - right).unwrap_or(0),
      u32::try_from(height - top - bottom).unwrap_or(0),
    )
  }
}
