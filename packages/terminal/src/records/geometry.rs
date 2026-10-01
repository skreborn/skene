//! What the last layout measured of a node, in cells.

use skene_foundation::Size;

use super::{Handle, Record, Records};

/// Where a laid-out node sits in its layout parent and how large it is, in cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CellFrame {
  pub(crate) x: i64,
  pub(crate) y: i64,
  pub(crate) width: i64,
  pub(crate) height: i64,
}

impl Records {
  /// `record`'s frame in its layout parent, as the last layout rounded it; empty for a
  /// record outside the layout.
  #[expect(
    clippy::cast_possible_truncation,
    reason = "taffy rounds a layout to whole cells, far inside i64"
  )]
  pub(crate) fn frame_of(&self, record: &Record) -> CellFrame {
    let Some(layout) = record.taffy.and_then(|taffy| self.taffy.layout(taffy).ok())
    else {
      return CellFrame::default();
    };

    CellFrame {
      x: layout.location.x as i64,
      y: layout.location.y as i64,
      width: layout.size.width.max(0.) as i64,
      height: layout.size.height.max(0.) as i64,
    }
  }

  /// `handle`'s frame in its layout parent; empty for a stale handle.
  pub(crate) fn frame(&self, handle: Handle) -> CellFrame {
    self
      .record(handle)
      .map_or_else(CellFrame::default, |record| self.frame_of(record))
  }

  /// A node's laid-out size, in cells.
  pub(crate) fn node_size(&self, handle: Handle) -> (i64, i64) {
    let frame = self.frame(handle);

    (frame.width, frame.height)
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
    let frame = self.frame(handle);

    (frame.x, frame.y)
  }

  /// A node's border widths, in cells: left, top, right, bottom.
  #[expect(
    clippy::cast_possible_truncation,
    reason = "taffy rounds a layout to whole cells, far inside i64"
  )]
  pub(crate) fn node_border(&self, handle: Handle) -> (i64, i64, i64, i64) {
    let Some(layout) = self
      .record(handle)
      .and_then(|record| record.taffy)
      .and_then(|taffy| self.taffy.layout(taffy).ok())
    else {
      return (0, 0, 0, 0);
    };

    (
      layout.border.left as i64,
      layout.border.top as i64,
      layout.border.right as i64,
      layout.border.bottom as i64,
    )
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
