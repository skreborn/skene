//! Client-side window chrome, drawn by Skene where the compositor draws none (mutter, weston,
//! an X11 window asked to go undecorated): the window's geometry inside a surface that also
//! holds its shadow, what a point of the surface hits (a resize edge, the title bar, a
//! title-bar button, the content), and how the window's states change the drawing.
//!
//! Coordinates: "surface" is the whole buffer (shadow included); "geometry" is the window
//! proper (`xdg_surface.set_window_geometry`), whose origin is [`Chrome::origin`] in the
//! surface. The content (Skene's root) starts below the title bar.

use std::rc::Rc;

use skene_foundation::{Corners, layout::Rect, text::AttributedText};
use vello_cpu::kurbo::BezPath;

use crate::{
  controls::mix,
  look::RunProps,
  paint::Canvas,
  palette::Palette,
  text::{Change, TextCell},
};

/// The shadow's extent outside the window geometry on a floating window.
pub const SHADOW: f32 = 24.;
/// How far outside the geometry a press still resizes.
pub const RESIZE_OUTSIDE: f32 = 12.;
/// How far inside the geometry a press still resizes.
pub const RESIZE_INSIDE: f32 = 3.;
/// The size of a corner's grip along each edge.
pub const CORNER: f32 = 24.;
/// The title bar's height.
pub const HEADER: f32 = 38.;
/// A floating window's corner radius.
pub const RADIUS: f32 = 10.;

/// The window's states, as the window manager reports them.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "one flag per state")]
pub struct States {
  /// Maximized.
  pub maximized: bool,
  /// Fullscreen.
  pub fullscreen: bool,
  /// The window has the seat's focus.
  pub activated: bool,
  /// Being resized interactively.
  pub resizing: bool,
  /// Hidden from view (minimized, on another workspace).
  pub suspended: bool,
  /// Tiled left, top, right, bottom.
  pub tiled: [bool; 4],
}

/// What the window manager says it offers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[expect(clippy::struct_excessive_bools, reason = "one flag per capability")]
pub struct Capabilities {
  /// A window menu.
  pub window_menu: bool,
  /// Maximizing.
  pub maximize: bool,
  /// Minimizing.
  pub minimize: bool,
  /// Fullscreen.
  pub fullscreen: bool,
}

impl Default for Capabilities {
  fn default() -> Self {
    Self {
      window_menu: true,
      maximize: true,
      minimize: true,
      fullscreen: true,
    }
  }
}

/// A resize edge.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edge {
  /// The top edge.
  Top,
  /// The bottom edge.
  Bottom,
  /// The left edge.
  Left,
  /// The top left corner.
  TopLeft,
  /// The bottom left corner.
  BottomLeft,
  /// The right edge.
  Right,
  /// The top right corner.
  TopRight,
  /// The bottom right corner.
  BottomRight,
}

/// A title-bar button.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
  /// Minimizes the window.
  Minimize,
  /// Maximizes or restores it.
  Maximize,
  /// Closes it.
  Close,
}

/// What a point of the surface hits.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hit {
  /// The shadow beyond the resize band: input passes through.
  Outside,
  /// A resize edge.
  Edge(Edge),
  /// The title bar.
  Title,
  /// A title-bar button.
  Button(Button),
  /// The content, in geometry coordinates.
  Content(f32, f32),
}

/// The chrome's state.
#[derive(Clone)]
pub struct Chrome {
  /// Whether Skene draws the decorations (the compositor took none).
  pub csd: bool,
  /// Whether a shadow is drawn outside the geometry (a translucent surface).
  pub shadow: bool,
  /// The window's states.
  pub states: States,
  /// What the window manager offers.
  pub capabilities: Capabilities,
  /// The button the pointer is over.
  pub hover: Option<Button>,
  /// The button a press started on, acted on at release over it.
  pub pressed: Option<Button>,
  title: Rc<TextCell>,
}

impl Default for Chrome {
  fn default() -> Self {
    Self {
      csd: false,
      shadow: true,
      states: States::default(),
      capabilities: Capabilities::default(),
      hover: None,
      pressed: None,
      title: TextCell::new(),
    }
  }
}

impl Chrome {
  /// Whether the window floats: no state pins any of its edges.
  pub fn floating(&self) -> bool {
    let states = &self.states;

    !states.maximized && !states.fullscreen && !states.tiled.iter().any(|tiled| *tiled)
  }

  /// The title bar's height: none where the compositor decorates or the window is
  /// fullscreen.
  pub fn header(&self) -> f32 {
    if self.csd && !self.states.fullscreen {
      HEADER
    } else {
      0.
    }
  }

  /// The shadow margins left, top, right, bottom.
  pub fn margins(&self) -> [f32; 4] {
    if !self.csd || !self.shadow || self.states.maximized || self.states.fullscreen {
      return [0.; 4];
    }

    let tiled = self.states.tiled;

    [0, 1, 2, 3].map(|side| if tiled[side] { 0. } else { SHADOW })
  }

  /// The geometry's origin in the surface.
  pub fn origin(&self) -> (f32, f32) {
    let margins = self.margins();

    (margins[0], margins[1])
  }

  /// The surface size for a geometry `width` by `height`.
  pub fn surface_size(&self, width: u32, height: u32) -> (u32, u32) {
    let margins = self.margins();

    (
      width + (margins[0] + margins[2]) as u32,
      height + (margins[1] + margins[3]) as u32,
    )
  }

  /// The input region in the surface: the geometry and the outer resize band.
  pub fn input_rect(&self, width: u32, height: u32) -> Rect {
    let (ox, oy) = self.origin();
    let band = if self.resizable() { RESIZE_OUTSIDE } else { 0. };
    let margins = self.margins();
    let left = ox - band.min(margins[0]);
    let top = oy - band.min(margins[1]);

    Rect {
      x: left,
      y: top,
      width: ox + width as f32 + band.min(margins[2]) - left,
      height: oy + height as f32 + band.min(margins[3]) - top,
    }
  }

  fn resizable(&self) -> bool {
    self.csd && !self.states.maximized && !self.states.fullscreen
  }

  /// The title-bar buttons shown, right to left, with their rectangles in geometry
  /// coordinates.
  pub fn buttons(&self, width: u32) -> Vec<(Button, Rect)> {
    if self.header() <= 0. {
      return Vec::new();
    }

    let mut shown = vec![Button::Close];

    if self.capabilities.maximize {
      shown.push(Button::Maximize);
    }

    if self.capabilities.minimize {
      shown.push(Button::Minimize);
    }

    let mut right = width as f32 - 8.;

    shown
      .into_iter()
      .map(|button| {
        let rect = Rect {
          x: right - 24.,
          y: (HEADER - 24.) / 2.,
          width: 24.,
          height: 24.,
        };

        right -= 32.;

        (button, rect)
      })
      .collect()
  }

  /// What the surface point `(x, y)` hits, for a geometry `width` by `height`.
  pub fn hit(&self, x: f32, y: f32, width: u32, height: u32) -> Hit {
    let (ox, oy) = self.origin();
    let (gx, gy) = (x - ox, y - oy);
    let (w, h) = (width as f32, height as f32);

    if self.resizable() {
      let tiled = self.states.tiled;
      let near = |distance: f32| (-RESIZE_OUTSIDE..RESIZE_INSIDE).contains(&distance);
      let left = !tiled[0] && near(gx);
      let top = !tiled[1] && near(gy);
      let right = !tiled[2] && near(w - gx);
      let bottom = !tiled[3] && near(h - gy);
      let inside_x = (-RESIZE_OUTSIDE..w + RESIZE_OUTSIDE).contains(&gx);
      let inside_y = (-RESIZE_OUTSIDE..h + RESIZE_OUTSIDE).contains(&gy);
      let corner_x = gx < CORNER || gx > w - CORNER;
      let corner_y = gy < CORNER || gy > h - CORNER;
      let edge = match (left, top, right, bottom) {
        _ if !(inside_x && inside_y) => None,
        (true, _, _, _) if gy < CORNER => Some(Edge::TopLeft),
        (true, _, _, _) if gy > h - CORNER => Some(Edge::BottomLeft),
        (true, _, _, _) => Some(Edge::Left),
        (_, _, true, _) if gy < CORNER => Some(Edge::TopRight),
        (_, _, true, _) if gy > h - CORNER => Some(Edge::BottomRight),
        (_, _, true, _) => Some(Edge::Right),
        (_, true, _, _) if gx < CORNER && corner_y => Some(Edge::TopLeft),
        (_, true, _, _) if gx > w - CORNER && corner_y => Some(Edge::TopRight),
        (_, true, _, _) => Some(Edge::Top),
        (_, _, _, true) if gx < CORNER && corner_x => Some(Edge::BottomLeft),
        (_, _, _, true) if gx > w - CORNER && corner_x => Some(Edge::BottomRight),
        (_, _, _, true) => Some(Edge::Bottom),
        _ => None,
      };

      if let Some(edge) = edge {
        return Hit::Edge(edge);
      }
    }

    if gx < 0. || gy < 0. || gx >= w || gy >= h {
      return Hit::Outside;
    }

    if gy < self.header() {
      for (button, rect) in self.buttons(width) {
        if gx >= rect.x && gx < rect.x + rect.width && gy >= rect.y && gy < rect.y + rect.height {
          return Hit::Button(button);
        }
      }

      return Hit::Title;
    }

    Hit::Content(gx, gy)
  }

  /// The radius of the window's top corners: rounded only while it floats with CSD.
  pub fn radius(&self) -> f32 {
    if self.csd && self.floating() {
      RADIUS
    } else {
      0.
    }
  }

  /// Shows `title` in the title bar.
  pub fn set_title(&self, title: &str, palette: &Palette) -> Change {
    let props = RunProps {
      weight: Some(skene_foundation::FontWeight::BOLD),
      ..RunProps::default()
    };

    self.title.set(
      AttributedText::plain(title, props.run_style(palette.ink, 14.)),
      props.paragraph_style(),
    )
  }

  /// Paints what lies behind and around the content of a geometry `width` by `height`: the
  /// shadow, the window's fill (rounded at the top where it floats), the title bar.
  pub fn paint(&self, canvas: &mut Canvas<'_>, width: u32, height: u32) {
    let palette = canvas.palette;
    let (w, h) = (width as f32, height as f32);
    let window = Rect {
      x: 0.,
      y: 0.,
      width: w,
      height: h,
    };
    let radius = self.radius();
    let corners = Corners {
      top_leading: radius,
      top_trailing: radius,
      bottom_trailing: 0.,
      bottom_leading: 0.,
    };

    if self.csd && self.margins() != [0.; 4] {
      let (alpha, blur, drop) = if self.states.activated {
        (90, 16., 3.)
      } else {
        (46, 10., 1.)
      };

      canvas.shadow(
        window,
        Corners::all(radius),
        0.,
        drop,
        blur,
        0.,
        skene_foundation::Color::BLACK.with_alpha(alpha),
      );
      canvas.shadow(
        window,
        Corners::all(radius + 1.),
        0.,
        0.,
        1.,
        1.,
        skene_foundation::Color::BLACK.with_alpha(30),
      );
    }

    canvas.fill(window, corners, palette.window);

    let header = self.header();

    if header <= 0. {
      return;
    }

    let bar = Rect {
      height: header,
      ..window
    };
    let fill = if self.states.activated {
      palette.header
    } else {
      mix(palette.header, palette.window, 0.5)
    };

    canvas.fill(bar, corners, fill);
    canvas.fill(
      Rect {
        x: 0.,
        y: header - 1.,
        width: w,
        height: 1.,
      },
      Corners::all(0.),
      palette.outline.with_alpha(120),
    );

    if let Some(laid) = self.title.laid(None) {
      let size = skene_foundation::text::Laid::size(&laid);

      canvas.text(
        &laid,
        (((w - size.width) / 2.).max(8.), (header - size.height) / 2.),
      );
    }

    for (button, rect) in self.buttons(width) {
      let center = (rect.x + rect.width / 2., rect.y + rect.height / 2.);

      if self.hover == Some(button) || self.pressed == Some(button) {
        let fill = if self.pressed == Some(button) {
          palette.control_pressed
        } else {
          palette.control_hover
        };

        canvas.circle(center, 12., fill);
      }

      let mut icon = BezPath::new();
      let (x, y) = (f64::from(center.0), f64::from(center.1));

      match button {
        Button::Close => {
          icon.move_to((x - 4., y - 4.));
          icon.line_to((x + 4., y + 4.));
          icon.move_to((x + 4., y - 4.));
          icon.line_to((x - 4., y + 4.));
        }
        Button::Maximize if self.states.maximized => {
          icon.move_to((x - 3., y - 5.));
          icon.line_to((x + 5., y - 5.));
          icon.line_to((x + 5., y + 3.));
          icon.move_to((x - 5., y - 2.));
          icon.line_to((x + 2., y - 2.));
          icon.line_to((x + 2., y + 5.));
          icon.line_to((x - 5., y + 5.));
          icon.close_path();
        }
        Button::Maximize => {
          icon.move_to((x - 4.5, y - 4.5));
          icon.line_to((x + 4.5, y - 4.5));
          icon.line_to((x + 4.5, y + 4.5));
          icon.line_to((x - 4.5, y + 4.5));
          icon.close_path();
        }
        Button::Minimize => {
          icon.move_to((x - 4.5, y + 3.5));
          icon.line_to((x + 4.5, y + 3.5));
        }
      }

      canvas.stroke_path(&icon, 1.5, palette.ink);
    }
  }
}
