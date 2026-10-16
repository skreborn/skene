//! A picker's open list: a surface of its own beside the window (an `xdg_popup` on Wayland,
//! an override-redirect window on X11), placed below the picker's face, drawn by Skene, its
//! rows highlighted by the keyboard and the pointer.

use std::rc::Rc;

use skene_foundation::{
  Corners,
  behavior::TypeAhead,
  layout::Rect,
  text::{AttributedText, Laid as _, Wrap},
  value::BorderStyle,
};
use vello_cpu::kurbo::BezPath;

use crate::{
  look::RunProps,
  node::Node,
  paint::Renderer,
  palette::{DEFAULT_SIZE, Palette},
  shell::Frame,
  text::TextCell,
};

/// A row's height.
const ROW: f32 = 30.;
/// The space around the rows.
const INSET: f32 = 4.;
/// The space before a row's text, and after it beside the mark.
const PADDING: f32 = 12.;
/// The room the chosen option's mark takes at a row's end.
const MARK: f32 = 24.;
/// The tallest a list grows before it scrolls.
const MAX_HEIGHT: f32 = 320.;

/// Where a popup sits and how big it is, in the window's coordinates (its geometry's).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Placement {
  /// The rectangle it hangs from: the picker's face.
  pub anchor: Rect,
  /// Its width and height.
  pub size: (u32, u32),
}

/// A picker's open list.
pub struct OpenList {
  /// The picker it belongs to.
  pub owner: Node,
  /// The options' texts.
  pub options: Rc<[Rc<str>]>,
  /// The chosen option, as the picker showed it when it opened.
  pub chosen: Option<usize>,
  /// The highlighted option: where `Enter` chooses.
  pub highlighted: Option<usize>,
  /// What the reader has typed to find an option.
  pub type_ahead: TypeAhead,
  /// Where it sits.
  pub placement: Placement,
  scroll: f32,
  cells: Vec<Rc<TextCell>>,
  renderer: Renderer,
  dirty: bool,
}

impl OpenList {
  /// The list of `owner`'s options, their text in `inherited`, hanging from `anchor`.
  pub fn new(owner: Node, anchor: Rect, inherited: &RunProps, palette: &Palette) -> Self {
    let (options, chosen) = owner.contents.borrow().choice.as_ref().map_or_else(
      || (Rc::from([]), None),
      |choice| {
        (
          Rc::clone(&choice.options),
          choice.chosen.map(|index| index as usize),
        )
      },
    );
    let style = inherited.run_style(palette.ink, DEFAULT_SIZE);
    let paragraph = inherited.paragraph_style().with_wrap(Wrap::None);
    let cells: Vec<Rc<TextCell>> = options
      .iter()
      .map(|option| {
        let cell = TextCell::new();

        cell.set(
          AttributedText::plain(Rc::clone(option), style.clone()),
          paragraph,
        );

        cell
      })
      .collect();
    let widest = cells
      .iter()
      .map(|cell| cell.intrinsic().max)
      .fold(0., f32::max);
    let width = (widest + PADDING + MARK + INSET * 2.)
      .max(anchor.width)
      .ceil();
    let height = (ROW * options.len() as f32 + INSET * 2.)
      .min(MAX_HEIGHT)
      .ceil();
    let mut list = Self {
      owner,
      options,
      chosen,
      highlighted: chosen,
      type_ahead: TypeAhead::new(),
      placement: Placement {
        anchor,
        size: (width as u32, height.max(ROW) as u32),
      },
      scroll: 0.,
      cells,
      renderer: Renderer::new(),
      dirty: true,
    };

    if let Some(chosen) = chosen {
      list.reveal(chosen);
    }

    list
  }

  /// The rows' total height.
  fn extent(&self) -> f32 {
    ROW * self.options.len() as f32 + INSET * 2.
  }

  /// Row `index`'s rectangle, in the list's coordinates.
  pub fn row_rect(&self, index: usize) -> Rect {
    Rect {
      x: INSET,
      y: INSET + ROW * index as f32 - self.scroll,
      width: self.placement.size.0 as f32 - INSET * 2.,
      height: ROW,
    }
  }

  /// The row at `x`, `y` (the list's coordinates).
  pub fn row_at(&self, x: f32, y: f32) -> Option<usize> {
    let (width, height) = (self.placement.size.0 as f32, self.placement.size.1 as f32);

    if x < 0. || y < 0. || x >= width || y >= height {
      return None;
    }

    let row = ((y + self.scroll - INSET) / ROW).floor();

    (row >= 0. && (row as usize) < self.options.len()).then_some(row as usize)
  }

  /// Highlights `index`, scrolling it into view.
  pub fn highlight(&mut self, index: Option<usize>) {
    if self.highlighted != index {
      self.highlighted = index;
      self.dirty = true;
    }

    if let Some(index) = index {
      self.reveal(index);
    }
  }

  /// Scrolls row `index` into view.
  fn reveal(&mut self, index: usize) {
    let height = self.placement.size.1 as f32;
    let top = INSET + ROW * index as f32;
    let scroll = if top - INSET < self.scroll {
      top - INSET
    } else if top + ROW + INSET > self.scroll + height {
      top + ROW + INSET - height
    } else {
      self.scroll
    };
    let scroll = scroll.clamp(0., (self.extent() - height).max(0.));

    if (scroll - self.scroll).abs() > f32::EPSILON {
      self.scroll = scroll;
      self.dirty = true;
    }
  }

  /// Scrolls by `dy` pixels.
  pub fn scroll_by(&mut self, dy: f32) {
    let height = self.placement.size.1 as f32;
    let scroll = (self.scroll + dy).clamp(0., (self.extent() - height).max(0.));

    if (scroll - self.scroll).abs() > f32::EPSILON {
      self.scroll = scroll;
      self.dirty = true;
    }
  }

  /// The list needs painting again.
  pub fn invalidate(&mut self) {
    self.dirty = true;
  }

  /// Whether a frame is due.
  pub const fn wants_frame(&self) -> bool {
    self.dirty
  }

  /// Paints the list at `scale` where it changed: the whole surface.
  pub fn frame(&mut self, scale: f64) -> Option<Frame<'_>> {
    if !self.dirty {
      return None;
    }

    self.dirty = false;

    let (width, height) = self.placement.size;
    let device = (
      (f64::from(width) * scale).round() as u16,
      (f64::from(height) * scale).round() as u16,
    );
    let palette = Palette::now();
    let bounds = Rect {
      x: 0.,
      y: 0.,
      width: width as f32,
      height: height as f32,
    };
    let rows: Vec<(usize, Rect)> = (0..self.options.len())
      .map(|index| (index, self.row_rect(index)))
      .filter(|(_, rect)| rect.y + rect.height > 0. && rect.y < bounds.height)
      .collect();
    let (cells, highlighted, chosen) = (&self.cells, self.highlighted, self.chosen);

    self.renderer.paint(
      None,
      device,
      scale,
      bounds,
      (0., 0.),
      &crate::paint::Marks::default(),
      &mut |canvas| {
        canvas.fill(bounds, Corners::all(0.), palette.field);
        canvas.clipped(
          Rect {
            x: 1.,
            y: 1.,
            width: bounds.width - 2.,
            height: bounds.height - 2.,
          },
          |canvas| {
            for (index, rect) in &rows {
              if highlighted == Some(*index) {
                canvas.fill(*rect, Corners::all(6.), palette.control_hover);
              }

              if let Some(laid) = cells[*index].laid(None) {
                let size = laid.size();

                canvas.text(
                  &laid,
                  (
                    rect.x + PADDING - INSET,
                    rect.y + (rect.height - size.height) / 2.,
                  ),
                );
              }

              if chosen == Some(*index) {
                let (x, y) = (rect.x + rect.width - MARK + 4., rect.y + rect.height / 2.);
                let mut tick = BezPath::new();

                tick.move_to((f64::from(x), f64::from(y)));
                tick.line_to((f64::from(x + 4.), f64::from(y + 4.)));
                tick.line_to((f64::from(x + 11.), f64::from(y - 5.)));
                canvas.stroke_path(&tick, 2., palette.accent);
              }
            }
          },
        );
        canvas.border(
          bounds,
          Corners::all(0.),
          1.,
          BorderStyle::Solid,
          palette.outline,
        );
      },
    );

    Some(Frame {
      pixmap: &self.renderer.pixmap,
      damage: (0, 0, u32::from(device.0), u32::from(device.1)),
    })
  }

  /// The pixels last painted.
  pub const fn pixmap(&self) -> &vello_cpu::Pixmap {
    &self.renderer.pixmap
  }
}
