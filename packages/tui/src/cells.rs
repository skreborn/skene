//! The escape trait a low-level terminal view states as its bound, and the handle it
//! paints through.

use skene_core::{Color, NodeId, Platform, Point, Rect, Size};

use crate::{
  paint::{Buffer, terminal_rgb},
  text,
};

/// A cell painter: draws into the node's rectangle every time the node repaints.
///
/// The rectangle is the box's content area on the screen, in cells; the [`Cells`]
/// handle addresses it from its own origin.
pub(crate) type Painter = dyn Fn(&mut Cells<'_>, Rect<u32>);

/// A terminal platform that runs a painter inside a box.
///
/// The bound a low-level terminal view states: the view builds an ordinary
/// [`Container`](crate::Container) for its sizing and hands the platform a painter for
/// it, which the platform runs with the box's laid-out rectangle whenever the box
/// repaints, after the box's own background and border and before its children.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not paint cells",
  label = "a view bound on `CellPainter` exists on the terminal platform alone"
)]
pub trait CellPainter: Platform {
  /// Runs `painter` inside the box `node` on every repaint; a later call replaces the
  /// painter.
  fn paint(node: NodeId, painter: Box<Painter>);
}

/// A window onto the frame being painted: the cells of one box's content rectangle.
///
/// A cell is addressed from the rectangle's top-left corner. A write outside the
/// rectangle, or outside the clip a scrolling ancestor imposes, is dropped.
#[must_use]
pub struct Cells<'a> {
  buffer: &'a mut Buffer,
  origin: (isize, isize),
  size: Size<u32>,
}

impl std::fmt::Debug for Cells<'_> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Cells").finish_non_exhaustive()
  }
}

impl<'a> Cells<'a> {
  pub(crate) const fn new(buffer: &'a mut Buffer, origin: (isize, isize), size: Size<u32>) -> Self {
    Self {
      buffer,
      origin,
      size,
    }
  }

  /// The rectangle's size, in cells.
  #[inline(always)]
  pub const fn size(&self) -> Size<u32> {
    self.size
  }

  /// The rectangle on the screen: its origin in frame cells and its size.
  #[inline(always)]
  pub fn rect(&self) -> Rect<u32> {
    Rect::new(
      Point::new(
        u32::try_from(self.origin.0).unwrap_or(0),
        u32::try_from(self.origin.1).unwrap_or(0),
      ),
      self.size,
    )
  }

  /// The frame cell at (`x`, `y`) of the rectangle, when it is inside and painted.
  fn cell(&mut self, x: u32, y: u32) -> Option<&mut crate::paint::TerminalCell> {
    if x >= self.size.width || y >= self.size.height {
      return None;
    }

    self.buffer.cell_mut(
      self.origin.0 + isize::try_from(x).ok()?,
      self.origin.1 + isize::try_from(y).ok()?,
    )
  }

  /// Writes `character` at (`x`, `y`), keeping the cell's colors; a wide character
  /// takes the cell after it too, and one at the rectangle's edge is dropped.
  pub fn put(&mut self, x: u32, y: u32, character: char) {
    let mut bytes = [0; 4];
    let cluster = character.encode_utf8(&mut bytes);
    let width = text::width(cluster).max(1);

    if x + u32::try_from(width).unwrap_or(u32::MAX) > self.size.width || y >= self.size.height {
      return;
    }

    let (Ok(x), Ok(y)) = (isize::try_from(x), isize::try_from(y))
    else {
      return;
    };

    self
      .buffer
      .put_cluster(self.origin.0 + x, self.origin.1 + y, cluster, width, |_| {});
  }

  /// Sets the foreground at (`x`, `y`); a transparent color clears it.
  pub fn set_foreground(&mut self, x: u32, y: u32, color: Color) {
    let rgb = terminal_rgb(color);

    if let Some(cell) = self.cell(x, y) {
      cell.foreground = rgb;
    }
  }

  /// Sets the background at (`x`, `y`); a transparent color clears it.
  pub fn set_background(&mut self, x: u32, y: u32, color: Color) {
    let rgb = terminal_rgb(color);

    if let Some(cell) = self.cell(x, y) {
      cell.background = rgb;
    }
  }
}
