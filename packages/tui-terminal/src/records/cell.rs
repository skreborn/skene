//! What the terminal keeps per node in its record: a text entry's presentation, the
//! cells the node covered at the last paint, and what it paints beyond its style.

use std::{fmt, rc::Rc};

use skene_foundation::Inset;

use super::{CellRect, Field};
use crate::{cells::Painter, paint::Image};

/// The terminal's own state of one realized node, carried in its record
/// ([`Drawn::Node`](skene_layout::runtime::Drawn::Node)).
#[derive(Default)]
pub struct CellNode {
  /// A text entry's presentation of its value; `None` for every other node.
  pub(crate) field: Option<Box<Field>>,
  /// The screen rectangle the node covered at the last paint.
  pub(crate) painted: Option<CellRect>,
  /// The painter a low-level view runs inside the box on every repaint.
  pub(crate) painter: Option<Box<Painter>>,
  /// The picture a painted box draws through the terminal's graphics.
  pub(crate) picture: Option<PictureRecord>,
  /// The box's decoration painted in pixels beneath its cells, where the terminal
  /// shows graphics and the decoration asks for more than cells draw.
  pub(crate) backdrop: Option<Backdrop>,
}

impl fmt::Debug for CellNode {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("CellNode")
      .field("field", &self.field.is_some())
      .field("painted", &self.painted)
      .field("painter", &self.painter.is_some())
      .field("picture", &self.picture.is_some())
      .field("backdrop", &self.backdrop.is_some())
      .finish()
  }
}

/// A box's decoration as pixels: what it was rendered from, and the image.
pub(crate) struct Backdrop {
  /// What the image was rendered from.
  pub(crate) key: BackdropKey,
  /// The decoration's pixels.
  pub(crate) image: Rc<Image>,
}

/// What a backdrop is rendered from; a change to any part renders it afresh.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BackdropKey {
  /// The box's columns and the shadow's band.
  pub(crate) columns: u16,
  /// The box's rows and the shadow's band.
  pub(crate) rows: u16,
  /// The cells the shadow reaches down and right with its softening.
  pub(crate) band: u16,
  /// A cell's size in pixels.
  pub(crate) cell_pixels: (u16, u16),
  /// The box's surface; `None` for none.
  pub(crate) background: Option<(u8, u8, u8)>,
  /// The border's ink; `None` for no border or a transparent one.
  pub(crate) border: Option<(u8, u8, u8)>,
  /// The edges the border is drawn on.
  pub(crate) border_edges: Inset<bool>,
  /// The corner radius.
  pub(crate) radius: f32,
  /// The shadow's color as RGBA; `None` for no shadow or a transparent one.
  pub(crate) shadow: Option<[f32; 4]>,
  /// How far the shadow falls down and right, in cells.
  pub(crate) shadow_offset: f32,
  /// How far the shadow's edge softens past its offset, in cells.
  pub(crate) shadow_softening: f32,
}

/// A painted box's picture: its painter, and the image last rendered from it, dropped
/// when the painter is replaced and stale when the box's pixel size differs.
pub(crate) struct PictureRecord {
  /// The painter the picture is rendered from.
  pub(crate) painter: Rc<skene_foundation::Painter>,
  /// The image last rendered from the painter; `None` until one is.
  pub(crate) image: Option<Rc<Image>>,
}
