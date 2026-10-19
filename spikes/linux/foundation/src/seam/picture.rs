//! A painted node's picture: kurbo's shapes and peniko's brushes recorded once, in the device
//! pixels the node is drawn in, its top leading corner at the origin on the device's pixel
//! grid, and replayed by the realization in the scene's order, under the node's clip and
//! opacity.

/// The geometry a picture is drawn with.
pub use kurbo;
use kurbo::{Affine, BezPath, Stroke};
/// The paints a picture is drawn with.
pub use peniko;

use crate::Color;

/// What a shape is filled or stroked with.
#[derive(Clone, PartialEq, Debug)]
pub enum Paint {
  /// The node's text color, as it inherits it: a mark drawn in its control's ink.
  Ink,
  /// A color.
  Color(Color),
  /// A peniko brush: a solid color or a gradient (an image brush draws nothing).
  Brush(peniko::Brush),
}

impl From<Color> for Paint {
  #[inline(always)]
  fn from(color: Color) -> Self {
    Self::Color(color)
  }
}

impl From<peniko::Brush> for Paint {
  #[inline(always)]
  fn from(brush: peniko::Brush) -> Self {
    Self::Brush(brush)
  }
}

/// One step of a picture.
#[derive(Clone, PartialEq, Debug)]
pub enum PaintOp {
  /// Fills a path.
  Fill {
    /// The path, in the node's device pixels under the transform in force.
    path: BezPath,
    /// What it is filled with.
    paint: Paint,
  },
  /// Strokes a path.
  Stroke {
    /// The path, in the node's device pixels under the transform in force.
    path: BezPath,
    /// How the line is drawn.
    stroke: Stroke,
    /// What it is drawn with.
    paint: Paint,
  },
  /// Cuts what follows to a path, until the matching [`PaintOp::Pop`].
  PushClip(BezPath),
  /// Draws what follows at an opacity, until the matching [`PaintOp::Pop`].
  PushOpacity(f32),
  /// Ends the innermost clip or opacity.
  Pop,
  /// The transform the following paths are drawn under, from the node's device pixels.
  Transform(Affine),
}

/// A recording of paint operations, in the node's device pixels.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Picture {
  ops: Vec<PaintOp>,
}

impl Picture {
  /// An empty picture.
  #[inline(always)]
  #[must_use]
  pub const fn new() -> Self {
    Self { ops: Vec::new() }
  }

  /// Appends `op`.
  pub fn push(&mut self, op: PaintOp) {
    self.ops.push(op);
  }

  /// The operations, in order.
  #[inline(always)]
  #[must_use]
  pub fn ops(&self) -> &[PaintOp] {
    &self.ops
  }

  /// Whether it draws nothing.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.ops.is_empty()
  }
}
