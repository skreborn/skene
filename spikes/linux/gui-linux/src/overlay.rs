//! An open overlay: a popup beside the window (an `xdg_popup` on Wayland, an
//! override-redirect window on X11), placed against its anchor, its content Skene's own nodes
//! laid out apart from the window's at their natural size and painted by the same painter.
//!
//! An overlay node takes no room in its parent's layout and is drawn on no surface but its
//! own. Its frame is in window coordinates, so its content's window rectangles (what a reader
//! and an input method are told) are where the popup shows; the backend says where it placed
//! the popup once the compositor or the screen's edge has had its say.

use std::{
  cell::RefCell,
  rc::{Rc, Weak},
};

use skene_foundation::{
  Color,
  layout::{Axis, LayoutHost as _, Needed, Rect},
  seam::content::Side,
};

use crate::{
  layout::{self, Host},
  node::{Drawn, Node},
  paint::{Marks, Renderer},
  palette::Palette,
  shell::Frame,
};

/// Where a popup sits and how big it is, in the window's coordinates (its geometry's).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Placement {
  /// The rectangle it hangs from: its anchor's.
  pub anchor: Rect,
  /// Its width and height.
  pub size: (u32, u32),
  /// The space between it and its anchor.
  pub gap: f32,
}

thread_local! {
  static OVERLAYS: RefCell<Vec<Weak<Drawn>>> = const { RefCell::new(Vec::new()) };
}

/// Remembers `node`, an overlay, so it is opened once shown.
pub fn register(node: &Node) {
  OVERLAYS.with(|overlays| {
    let mut overlays = overlays.borrow_mut();

    overlays.retain(|overlay| overlay.strong_count() > 0);
    overlays.push(Rc::downgrade(node));
  });
}

/// The first overlay shown now, where one is.
pub fn shown() -> Option<Node> {
  OVERLAYS.with(|overlays| {
    overlays
      .borrow()
      .iter()
      .filter_map(Weak::upgrade)
      .find(|overlay| overlay.is_alive() && overlay.is_shown())
  })
}

/// The open overlay: its node, where it sits, its renderer and whether it is to be painted
/// again.
pub struct Open {
  /// The overlay node.
  pub node: Node,
  /// Where it sits.
  pub placement: Placement,
  renderer: Renderer,
  dirty: bool,
  opaque: bool,
  /// The scale its surface prefers, where the compositor told it one: its own, which differs
  /// from the window's where it shows on another output.
  scale: Option<f64>,
}

impl Open {
  /// `node`, laid out against its anchor.
  pub fn new(node: Node) -> Self {
    let placement = Self::lay_out(&node);

    Self {
      node,
      placement,
      renderer: Renderer::new(),
      dirty: true,
      opaque: false,
      scale: None,
    }
  }

  /// The scale its surface prefers, where the compositor told it one.
  #[must_use]
  pub const fn scale(&self) -> Option<f64> {
    self.scale
  }

  /// Its surface prefers `scale`: it is painted again at it.
  pub fn set_scale(&mut self, scale: f64) {
    if self.scale != Some(scale) {
      self.scale = Some(scale);
      self.dirty = true;
    }
  }

  /// Says whether its surface shows no transparency (an X11 window of the screen's own
  /// depth): its rounded corners are then filled with the window's color, what shows behind
  /// a popup on a desktop drawn in that color, rather than left clear.
  pub const fn set_opaque_surface(&mut self, opaque: bool) {
    self.opaque = opaque;
  }

  /// The node `node` is placed against: its placement's anchor, else the nearest realized
  /// node it stands in.
  fn anchor(node: &Node) -> Option<Node> {
    let placement = node.contents.borrow().placement;

    placement
      .and_then(|placement| placement.anchor)
      .and_then(crate::node_of)
      .or_else(|| node.parent())
  }

  /// Lays `node`'s content out at its natural size (as wide as its anchor at least where it
  /// widens to it) and places it on its anchor's side, or below the point it hangs from: where
  /// it sits.
  fn lay_out(node: &Node) -> Placement {
    let placement = node.contents.borrow().placement;
    let (side, widen) = placement.map_or((Side::Bottom, true), |placement| {
      (placement.side, placement.widen)
    });
    #[expect(clippy::cast_precision_loss, reason = "whole pixels")]
    let gap = placement.map_or(2., |placement| placement.gap as f32);
    let point = placement.and_then(|placement| placement.at);
    let anchor = match point {
      // A pixel just above the point, so what opens below it starts at the point.
      #[expect(clippy::cast_precision_loss, reason = "window pixels")]
      Some((x, y)) => Rect {
        x: x as f32,
        y: y as f32 - gap - 1.,
        width: 1.,
        height: 1.,
      },
      None => Self::anchor(node).map_or(Rect::default(), |anchor| anchor.window_rect()),
    };
    let (side, widen) = if point.is_some() {
      (Side::Bottom, false)
    } else {
      (side, widen)
    };

    layout::next_epoch();

    let natural = Host::measure(node, Axis::Horizontal, None, Needed::Natural).natural;
    let width = if widen {
      natural.max(anchor.width)
    } else {
      natural
    }
    .ceil()
    .max(1.);
    let height = Host::measure(node, Axis::Vertical, Some(width), Needed::Natural)
      .natural
      .ceil()
      .max(1.);
    let (x, y) = match side {
      Side::Bottom => (anchor.x, anchor.y + anchor.height + gap),
      Side::Top => (anchor.x, anchor.y - height - gap),
      Side::Leading => (anchor.x - width - gap, anchor.y),
      Side::Trailing => (anchor.x + anchor.width + gap, anchor.y),
    };

    node.layout_overlay(Rect {
      x,
      y,
      width,
      height,
    });
    layout::reach(node, 0);

    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "pixels"
    )]
    Placement {
      anchor,
      size: (width as u32, height as u32),
      gap,
    }
  }

  /// Lays its content out again: whether its size changed, so the popup is placed again.
  pub fn relayout(&mut self) -> bool {
    let frame = self.node.frame.get();
    let placement = Self::lay_out(&self.node);
    let moved = placement.size != self.placement.size;

    if !moved {
      // Where the backend placed it stays.
      self.node.frame.set(Rect {
        x: frame.x,
        y: frame.y,
        ..self.node.frame.get()
      });
    }

    self.placement = placement;
    self.dirty = true;

    moved
  }

  /// The backend placed the popup with its top left at `x`, `y` (window pixels).
  pub fn placed(&mut self, x: f32, y: f32) {
    let frame = self.node.frame.get();

    if (frame.x, frame.y) != (x, y) {
      self.node.frame.set(Rect { x, y, ..frame });
      crate::access::invalidate();
    }

    self.dirty = true;
  }

  /// Where it is: its top left as the backend placed it and its size (window pixels).
  pub fn rect(&self) -> Rect {
    self.node.frame.get()
  }

  /// Whether `x`, `y` (window pixels) is on it.
  pub fn contains(&self, x: f32, y: f32) -> bool {
    let frame = self.node.frame.get();

    x >= frame.x && y >= frame.y && x < frame.x + frame.width && y < frame.y + frame.height
  }

  /// Whether `x`, `y` (window pixels) is on its anchor.
  pub fn on_anchor(&self, x: f32, y: f32) -> bool {
    let anchor = self.placement.anchor;

    x >= anchor.x && y >= anchor.y && x < anchor.x + anchor.width && y < anchor.y + anchor.height
  }

  /// It is to be painted again.
  pub const fn invalidate(&mut self) {
    self.dirty = true;
  }

  /// Whether it is to be painted.
  pub const fn wants_frame(&self) -> bool {
    self.dirty
  }

  /// Its last frame's pixels: on the GPU path, read back from the GPU first.
  pub fn pixmap(&mut self) -> &vello_cpu::Pixmap {
    self.renderer.last_frame()
  }

  /// Its size in device pixels at `scale`.
  #[must_use]
  pub fn device_size(&self, scale: f64) -> (u16, u16) {
    let (width, height) = self.placement.size;

    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "pixels"
    )]
    (
      (f64::from(width) * scale).round() as u16,
      (f64::from(height) * scale).round() as u16,
    )
  }

  /// Paints on the GPU from now on, through `gpu`, presenting to its surface.
  #[cfg(feature = "gpu")]
  pub fn use_gpu(&mut self, gpu: crate::gpu::Gpu) {
    self.renderer.use_gpu(gpu);
    self.dirty = true;
  }

  /// Paints on the CPU from now on: the surface the GPU path presented to is going.
  pub fn drop_gpu(&mut self) {
    #[cfg(feature = "gpu")]
    self.renderer.drop_gpu();
  }

  /// Whether it paints on the GPU, which presents each frame itself.
  #[must_use]
  pub const fn presents(&self) -> bool {
    matches!(self.renderer.path(), crate::paint::PaintPath::Gpu)
  }

  /// Paints it whole at `scale`, where it is to be painted.
  pub fn frame(&mut self, scale: f64, marks: &Marks) -> Option<Frame<'_>> {
    if !std::mem::take(&mut self.dirty) {
      return None;
    }

    let frame = self.node.frame.get();
    let (width, height) = self.placement.size;
    let device = self.device_size(scale);
    let region = Rect {
      x: frame.x,
      y: frame.y,
      width: width as f32,
      height: height as f32,
    };
    let window = Palette::now().window;
    let opaque = self.opaque;

    let cost = self.renderer.paint(
      Some(&self.node),
      device,
      scale,
      region,
      (-frame.x, -frame.y),
      marks,
      &mut |canvas| {
        if opaque {
          canvas.fill(
            region,
            skene_foundation::Corners::all(0.),
            without_alpha(window),
          );
        }
      },
    );

    Some(Frame {
      pixmap: &self.renderer.pixmap,
      damage: (0, 0, u32::from(device.0), u32::from(device.1)),
      presented: cost.presented,
    })
  }
}

/// `color` with no transparency: what shows behind an opaque popup's rounded corners.
const fn without_alpha(color: Color) -> Color {
  Color {
    alpha: 255,
    ..color
  }
}
