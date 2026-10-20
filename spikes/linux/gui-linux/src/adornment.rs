//! What the window shows on small surfaces of its own beside it, which take no input, as GTK 4
//! shows them: the icon a drag of a text entry's selection carries under the pointer, and, for
//! a touchscreen, a text entry's selection handles and the magnifier above the text a finger
//! moves the caret or a selection's end in.

use std::rc::Rc;

use skene_foundation::{
  Color, Corners, Fill,
  layout::Rect,
  text::{
    AttributedText, Laid as _, Paragraph as _, ParagraphStyle, RunStyle, TextEngine as _, Wrap,
  },
};
use skene_parley::ParleyLaid;
use vello_cpu::kurbo::{Arc, BezPath, Point, Vec2};

use crate::{node::Node, paint::Canvas, palette::Palette, text};

/// One of the window's adornments.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Adornment {
  /// The icon a drag from the window shows under the pointer.
  DragIcon,
  /// A touch selection handle.
  Handle(Handle),
  /// The magnifier a finger moving the caret or a selection's end shows above the text.
  Magnifier,
}

/// Where an adornment shows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Placement {
  /// Under the pointer, which is at `hotspot` (window pixels from its top left) in it.
  Pointer {
    /// Where the pointer is in it.
    hotspot: (f32, f32),
  },
  /// Hanging below the rectangle (window pixels), centered on it: flipped above it where it
  /// does not fit on the output below, slid along to stay on it.
  Below(Rect),
  /// Standing above the rectangle (window pixels), centered on it: flipped below it where it
  /// does not fit on the output above, slid along to stay on it.
  Above(Rect),
}

impl Placement {
  /// This placement in another space: every length times `scale`, every position then moved
  /// by `offset`.
  #[must_use]
  pub fn transformed(self, scale: f32, offset: (f32, f32)) -> Self {
    let map = |rect: Rect| Rect {
      x: rect.x * scale + offset.0,
      y: rect.y * scale + offset.1,
      width: rect.width * scale,
      height: rect.height * scale,
    };

    match self {
      Self::Pointer { hotspot } => Self::Pointer {
        hotspot: (hotspot.0 * scale, hotspot.1 * scale),
      },
      Self::Below(anchor) => Self::Below(map(anchor)),
      Self::Above(anchor) => Self::Above(map(anchor)),
    }
  }

  /// The top left of an adornment `size` big placed so within `bounds`, all in one space (one
  /// under the pointer, from the pointer): what a backend that places its surfaces itself
  /// (X11) computes, as a compositor places a popup that may flip vertically and slide
  /// horizontally.
  #[must_use]
  pub fn origin(self, size: (f32, f32), bounds: Rect) -> (f32, f32) {
    let (width, height) = size;
    let (anchor, below) = match self {
      Self::Pointer { hotspot } => return (-hotspot.0, -hotspot.1),
      Self::Below(anchor) => (anchor, true),
      Self::Above(anchor) => (anchor, false),
    };
    let under = anchor.y + anchor.height;
    let over = anchor.y - height;
    let fits_under = under + height <= bounds.y + bounds.height;
    let fits_over = over >= bounds.y;
    let y = if below {
      if !fits_under && fits_over {
        over
      } else {
        under
      }
    } else if !fits_over && fits_under {
      under
    } else {
      over
    };
    let x = (anchor.x + anchor.width / 2. - width / 2.)
      .min(bounds.x + bounds.width - width)
      .max(bounds.x);

    (x, y)
  }
}

/// An adornment the window shows now.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shown {
  /// Which.
  pub adornment: Adornment,
  /// Its size, in window pixels.
  pub size: (u32, u32),
  /// Where it shows.
  pub placement: Placement,
  /// Changes whenever what it shows does: it is painted again.
  pub version: u64,
}

/// The widest a drag icon's text is laid out, in window pixels: GTK's.
const ICON_WIDTH: f32 = 250.;

/// The most lines a drag icon shows: from more, the first three, an ellipsis and the last
/// three, as GTK shows them.
const ICON_LINES: usize = 7;

/// The icon a drag of a text entry's selection carries, as GTK's entry makes it: the text in
/// the entry's style, wrapped where a word (or, in a word too wide, a character) meets 250
/// pixels, seven lines at most; on the entry's fill where the screen shows nothing through a
/// surface, on nothing elsewhere.
pub struct DragIcon {
  laid: ParleyLaid,
  size: (f32, f32),
  hotspot: (f32, f32),
  fill: Option<(Fill, Corners)>,
}

impl DragIcon {
  /// The icon of `selected` in `style`, the pointer at `hotspot` in it, on `fill` where the
  /// screen composites nothing.
  pub fn new(
    selected: &str,
    style: &RunStyle,
    hotspot: (f32, f32),
    fill: Option<(Fill, Corners)>,
  ) -> Self {
    let lay = |value: &str, width: Option<f32>| {
      text::engine()
        .paragraph(
          &AttributedText::plain(value, style.clone()),
          &ParagraphStyle::default().with_wrap(Wrap::Words),
        )
        .layout(width)
    };
    let width = lay(selected, None).size().width.clamp(1., ICON_WIDTH);
    let mut laid = lay(selected, Some(width));
    let lines = laid.lines();

    if lines.len() >= ICON_LINES {
      let text_of = |lines: &[skene_foundation::text::LineInfo]| {
        lines
          .iter()
          .map(|line| selected[line.range.clone()].trim_end_matches(['\n', '\r']))
          .collect::<String>()
      };
      let half = ICON_LINES / 2;
      let limited = format!(
        "{}\n\u{2026}\n{}",
        text_of(&lines[..half]),
        text_of(&lines[lines.len() - half..])
      );

      laid = lay(&limited, Some(width));
    }

    let size = laid.size();

    Self {
      laid,
      size: (size.width.ceil().max(1.), size.height.ceil().max(1.)),
      hotspot,
      fill,
    }
  }

  /// Its size, in window pixels.
  #[must_use]
  pub fn size(&self) -> (u32, u32) {
    (self.size.0 as u32, self.size.1 as u32)
  }

  /// Where the pointer is in it, in window pixels from its top left.
  #[must_use]
  pub const fn hotspot(&self) -> (f32, f32) {
    self.hotspot
  }

  /// Paints it at the canvas's top left: its fill first where `opaque`.
  pub fn paint(&self, canvas: &mut Canvas<'_>, opaque: bool) {
    if opaque && let Some((fill, corners)) = &self.fill {
      canvas.fill_with(
        Rect {
          x: 0.,
          y: 0.,
          width: self.size.0,
          height: self.size.1,
        },
        *corners,
        fill,
      );
    }

    canvas.text(&self.laid, (0., 0.));
  }
}

/// A touch selection handle, by what it moves.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Handle {
  /// The caret, where nothing is selected.
  Caret,
  /// The selection's start.
  Start,
  /// The selection's end.
  End,
}

/// A handle's icon, in window pixels: GTK's cursor-handle.
const HANDLE_ICON: (f32, f32) = (20., 24.);

/// The room around a handle's icon on its surface, either side and above and below: GTK's
/// cursor-handle padding.
const HANDLE_ROOM: (f32, f32) = (20., 24.);

/// A handle's surface, in window pixels.
pub const HANDLE_SIZE: (u32, u32) = (60, 72);

impl Handle {
  /// The rectangle its surface hangs below, pointing at `x` on a line from `top`, `height`
  /// tall (window pixels): its icon's width, centered on `x` for the caret, ending at it for
  /// the start, starting at it for the end, its icon's top a pixel below the line.
  #[must_use]
  pub fn anchor(self, x: f32, top: f32, height: f32) -> Rect {
    let left = match self {
      Self::Caret => x - HANDLE_ICON.0 / 2.,
      Self::Start => x - HANDLE_ICON.0,
      Self::End => x,
    };

    Rect {
      x: left,
      y: top + height - HANDLE_ROOM.1,
      width: HANDLE_ICON.0,
      height: 1.,
    }
  }

  /// Whether a finger `dx`, `dy` from where it points (its `x` and its line's top) takes it:
  /// within its icon and the room around it, measured from the line's top, as GTK measures.
  #[must_use]
  pub fn takes(self, dx: f32, dy: f32) -> bool {
    let (left, right) = match self {
      Self::Caret => (
        -HANDLE_ICON.0 / 2. - HANDLE_ROOM.0,
        HANDLE_ICON.0 / 2. + HANDLE_ROOM.0,
      ),
      Self::Start => (-HANDLE_ICON.0 - HANDLE_ROOM.0, HANDLE_ROOM.0),
      Self::End => (-HANDLE_ROOM.0, HANDLE_ICON.0 + HANDLE_ROOM.0),
    };

    dx >= left && dx < right && dy >= -HANDLE_ROOM.1 && dy < HANDLE_ICON.1 + HANDLE_ROOM.1
  }

  /// Its icon's outline on its surface: a drop with its point up at the caret, up and right
  /// at the start, up and left at the end.
  fn outline(self) -> BezPath {
    let (left, top) = HANDLE_ROOM;
    let radius = f64::from(HANDLE_ICON.0) / 2.;
    let center = Point::new(
      f64::from(left) + radius,
      f64::from(top + HANDLE_ICON.1) - radius,
    );
    let tip = match self {
      Self::Caret => Point::new(center.x, f64::from(top)),
      Self::Start => Point::new(f64::from(left + HANDLE_ICON.0), f64::from(top)),
      Self::End => Point::new(f64::from(left), f64::from(top)),
    };
    // The two points where lines from the tip touch the circle.
    let toward = tip - center;
    let distance = toward.hypot();
    let spread = (radius / distance).acos();
    let angle = toward.atan2();
    let mut path = BezPath::new();

    path.move_to(tip);
    path.line_to(center + Vec2::from_angle(angle + spread) * radius);
    path.extend(
      Arc::new(
        center,
        Vec2::new(radius, radius),
        angle + spread,
        std::f64::consts::TAU - 2. * spread,
        0.,
      )
      .append_iter(0.1),
    );
    path.close_path();

    path
  }

  /// Paints its surface: the icon in a control's fill, outlined.
  pub fn paint(self, canvas: &mut Canvas<'_>) {
    let palette = canvas.palette;
    let outline = self.outline();

    canvas.fill_path(&outline, palette.control);
    canvas.stroke_path(&outline, 1., palette.outline);
  }
}

/// The magnifier's view of the text, in window pixels: GTK's.
const MAGNIFIER_VIEW: (f32, f32) = (100., 60.);

/// How much the magnifier magnifies.
const MAGNIFICATION: f32 = 2.;

/// The space between the magnifier's frame and its view: the popover's border and padding.
const MAGNIFIER_INSET: f32 = 9.;

/// The tail the magnifier's frame points at the text with: its base and its height.
const MAGNIFIER_TAIL: (f32, f32) = (24., 12.);

/// The magnifier's surface, in window pixels: its frame and the tail below it.
pub const MAGNIFIER_SIZE: (u32, u32) = (118, 90);

/// The magnifier of a text entry: the text about a point shown twice as large in a frame
/// standing above the line, as GTK's entry shows it while a finger moves its caret or a
/// selection's end.
pub struct Magnifier {
  /// The entry it magnifies.
  pub entry: Node,
  /// The point it shows at its middle, in window pixels.
  pub center: (f32, f32),
  /// The line it stands above (window pixels): the tail points at its top.
  pub line: Rect,
}

impl Magnifier {
  /// Where it shows: above its line, centered on its point.
  #[must_use]
  pub const fn placement(&self) -> Placement {
    Placement::Above(Rect {
      x: self.center.0,
      y: self.line.y,
      width: 1.,
      height: self.line.height,
    })
  }

  /// Paints its surface: the frame (an on-screen display's translucent dark, with a faint
  /// border), and in it the entry's text about its point, magnified, on the field's fill.
  pub fn paint(&self, canvas: &mut Canvas<'_>, focused: bool) {
    let palette = canvas.palette;
    let frame = frame_outline();

    canvas.fill_path(&frame, osd(&palette));
    canvas.stroke_path(&frame, 1., Color::rgba(0xffff_ff1a));

    let view = Rect {
      x: MAGNIFIER_INSET,
      y: MAGNIFIER_INSET,
      width: MAGNIFIER_VIEW.0,
      height: MAGNIFIER_VIEW.1,
    };

    canvas.fill(view, Corners::all(0.), palette.field);

    let entry = Rc::clone(&self.entry);
    let rect = entry.window_rect();
    let middle = (view.x + view.width / 2., view.y + view.height / 2.);

    canvas.clipped(view, |canvas| {
      canvas.magnified(MAGNIFICATION, self.center, middle, |canvas| {
        entry.paint_entry(canvas, rect, focused);
      });
    });
  }
}

/// The on-screen display's fill: a translucent dark gray, darker in a dark scheme.
fn osd(palette: &Palette) -> Color {
  let window = palette.window;
  let dark = u16::from(window.red) + u16::from(window.green) + u16::from(window.blue) < 384;

  if dark {
    Color::rgba(0x2525_25b3)
  } else {
    Color::rgba(0x3535_35b3)
  }
}

/// The magnifier's frame on its surface: a rounded box, its tail out of the middle of its
/// bottom edge, half a pixel in so its border falls on whole pixels.
fn frame_outline() -> BezPath {
  let (width, height) = (
    f64::from(MAGNIFIER_SIZE.0) - 0.5,
    f64::from(MAGNIFIER_SIZE.1) - f64::from(MAGNIFIER_TAIL.1),
  );
  let (left, top) = (0.5, 0.5);
  let radius = 9.;
  let middle = f64::from(MAGNIFIER_SIZE.0) / 2.;
  let base = f64::from(MAGNIFIER_TAIL.0) / 2.;
  let tip = f64::from(MAGNIFIER_SIZE.1) - 0.5;
  let corner = |path: &mut BezPath, center: (f64, f64), start: f64| {
    path.extend(
      Arc::new(
        center,
        Vec2::new(radius, radius),
        start,
        std::f64::consts::FRAC_PI_2,
        0.,
      )
      .append_iter(0.1),
    );
  };
  let mut path = BezPath::new();

  path.move_to((left + radius, top));
  path.line_to((width - radius, top));
  corner(
    &mut path,
    (width - radius, top + radius),
    -std::f64::consts::FRAC_PI_2,
  );
  path.line_to((width, height - radius));
  corner(&mut path, (width - radius, height - radius), 0.);
  path.line_to((middle + base, height));
  path.line_to((middle, tip));
  path.line_to((middle - base, height));
  path.line_to((left + radius, height));
  corner(
    &mut path,
    (left + radius, height - radius),
    std::f64::consts::FRAC_PI_2,
  );
  path.line_to((left, top + radius));
  corner(
    &mut path,
    (left + radius, top + radius),
    std::f64::consts::PI,
  );
  path.close_path();

  path
}
