//! What the last layout measured of a node, in cells.

use skene_foundation::{NodeId, Rect, Size};

use super::Records;

/// Where a laid-out node sits in its layout parent and how large it is, in cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CellFrame {
  pub(crate) x: i64,
  pub(crate) y: i64,
  pub(crate) width: i64,
  pub(crate) height: i64,
}

impl CellFrame {
  /// The frame of `rect`, a rectangle the last layout rounded to whole cells.
  #[expect(
    clippy::cast_possible_truncation,
    reason = "taffy rounds a layout to whole cells, far inside i64"
  )]
  const fn of(rect: Rect<f32>) -> Self {
    Self {
      x: rect.origin.x as i64,
      y: rect.origin.y as i64,
      width: rect.size.width.max(0.) as i64,
      height: rect.size.height.max(0.) as i64,
    }
  }
}

impl Records {
  /// `node`'s frame in its layout parent, as the last layout rounded it; empty for a
  /// node outside the layout.
  pub(crate) fn frame(&self, node: NodeId) -> CellFrame {
    self
      .runtime
      .tree()
      .layout(node)
      .map_or_else(CellFrame::default, |layout| CellFrame::of(layout.rect))
  }

  /// A node's laid-out size, in cells.
  pub(crate) fn node_size(&self, node: NodeId) -> (i64, i64) {
    let frame = self.frame(node);

    (frame.width, frame.height)
  }

  /// A childless box's laid-out span in eighth cells from its parent's origin: where
  /// it begins and how far it extends, unrounded, so boxes beside each other meet
  /// where the layout put them. `None` for a box with laid-out children, those inside
  /// an anchor or a scope among them, or one of no width.
  pub(crate) fn smooth_span(&self, node: NodeId) -> Option<(isize, isize)> {
    let mut beneath = self.runtime.children(node);

    while let Some(child) = beneath.pop() {
      if self.runtime.contains(child) {
        return None;
      }

      beneath.extend(self.runtime.children(child));
    }

    let rect = self.runtime.tree().unrounded_layout(node)?.rect;
    #[expect(
      clippy::cast_possible_truncation,
      reason = "a position in eighth cells stays far inside isize"
    )]
    let start = (rect.origin.x * 8.).round() as isize;
    #[expect(
      clippy::cast_possible_truncation,
      reason = "an extent in eighth cells stays far inside isize"
    )]
    let extent = (rect.size.width.max(0.) * 8.).round() as isize;

    (extent > 0).then_some((start, extent))
  }

  /// A node's border widths, in cells: left, top, right, bottom.
  #[expect(
    clippy::cast_possible_truncation,
    reason = "taffy rounds a layout to whole cells, far inside i64"
  )]
  pub(crate) fn node_border(&self, node: NodeId) -> (i64, i64, i64, i64) {
    let Some(layout) = self.runtime.tree().layout(node)
    else {
      return (0, 0, 0, 0);
    };
    let border = layout.border;

    (
      border.start as i64,
      border.top as i64,
      border.end as i64,
      border.bottom as i64,
    )
  }

  /// A node's size inside its border, in cells: what its size observers hear.
  pub(crate) fn client_size(&self, node: NodeId) -> Size<i32> {
    let (width, height) = self.node_size(node);
    let (left, top, right, bottom) = self.node_border(node);
    let cells = |extent: i64| i32::try_from(extent.max(0)).unwrap_or(i32::MAX);

    Size::new(cells(width - left - right), cells(height - top - bottom))
  }
}
