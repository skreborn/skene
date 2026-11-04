//! What the last layout measured of a node, in cells.

use skene_foundation::{NodeId, Rect, Size};
use skene_layout::runtime::Flow;

use super::Records;
use crate::CellEngine;

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

  /// Whether the cell `point` lies inside the frame's size placed with its top left at the
  /// cell `origin`, its location aside.
  pub(crate) const fn covers(&self, origin: (i64, i64), point: (i64, i64)) -> bool {
    point.0 >= origin.0
      && point.0 < origin.0 + self.width
      && point.1 >= origin.1
      && point.1 < origin.1 + self.height
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

  /// `node`'s frame in its layout parent, as [`frame`](Self::frame), and the
  /// cells its laid-out content reaches from the frame's origin, left, top, right and
  /// bottom: its own box, or farther where its content overflows it.
  pub(crate) fn reach_of(&self, node: NodeId) -> (CellFrame, [f32; 4]) {
    let Some(layout) = self.runtime.tree().layout(node)
    else {
      return (CellFrame::default(), [0.; 4]);
    };
    let size = layout.rect.size;
    let (left, top) = (
      layout.border.start + layout.overflow.origin.x,
      layout.border.top + layout.overflow.origin.y,
    );

    (
      CellFrame::of(layout.rect),
      [
        left.min(0.),
        top.min(0.),
        (left + layout.overflow.size.width).max(size.width),
        (top + layout.overflow.size.height).max(size.height),
      ],
    )
  }

  /// A node's laid-out size, in cells.
  pub(crate) fn node_size(&self, handle: NodeId) -> (i64, i64) {
    let frame = self.frame(handle);

    (frame.width, frame.height)
  }

  /// The cells the text node `handle` lays its flow out across, as its leaf measured and
  /// placed it: its content box's width before any rounding, floored to whole cells, at
  /// least one; one for a node outside the layout.
  pub(crate) fn flow_width(&self, handle: NodeId) -> usize {
    let width = self
      .runtime
      .tree()
      .unrounded_layout(handle)
      .map_or(0., |layout| {
        layout.rect.size.width
          - layout.border.start
          - layout.border.end
          - layout.padding.start
          - layout.padding.end
      });

    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "a whole, positive count of cells, far inside usize"
    )]
    let cells = Flow::<CellEngine>::whole_width(width) as usize;

    cells
  }

  /// A childless box's laid-out span in eighth cells from its parent's origin: where
  /// it begins and how far it extends, unrounded, so boxes beside each other meet
  /// where the layout put them. `None` for a box with laid-out children, those inside
  /// an anchor or a scope among them, or one of no width.
  pub(crate) fn smooth_span(&self, handle: NodeId) -> Option<(isize, isize)> {
    let mut beneath = self.runtime.children(handle);

    while let Some(child) = beneath.pop() {
      if self.runtime.contains(child) {
        return None;
      }

      beneath.extend(self.runtime.children(child));
    }

    let rect = self.runtime.tree().unrounded_layout(handle)?.rect;
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

  /// A node's laid-out position within its layout parent, in cells.
  pub(crate) fn node_location(&self, handle: NodeId) -> (i64, i64) {
    let frame = self.frame(handle);

    (frame.x, frame.y)
  }

  /// A node's border widths, in cells: left, top, right, bottom.
  #[expect(
    clippy::cast_possible_truncation,
    reason = "taffy rounds a layout to whole cells, far inside i64"
  )]
  pub(crate) fn node_border(&self, handle: NodeId) -> (i64, i64, i64, i64) {
    let Some(layout) = self.runtime.tree().layout(handle)
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
  pub(crate) fn client_size(&self, handle: NodeId) -> Size<i32> {
    let (width, height) = self.node_size(handle);
    let (left, top, right, bottom) = self.node_border(handle);
    let cells = |extent: i64| i32::try_from(extent.max(0)).unwrap_or(i32::MAX);

    Size::new(cells(width - left - right), cells(height - top - bottom))
  }
}
