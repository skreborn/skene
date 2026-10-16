//! A box the app draws: a recording of kurbo's shapes and peniko's brushes the platform replays
//! in the scene's order, recorded again whenever what its closure read changes or the box is
//! laid out at another size.

use std::rc::Rc;

pub use skene_foundation::seam::picture::Paint;
/// The geometry a [`Canvas`] draws with (kurbo).
pub use skene_foundation::seam::picture::kurbo as shapes;
/// The paints a [`Canvas`] draws with (peniko), beside the node's own ink ([`Paint::Ink`]).
pub use skene_foundation::seam::picture::peniko as paints;
use skene_foundation::{
  Color, Dimension, Memo, Signal, StateSet,
  seam::{
    access::Role,
    content::contents,
    node::{NodeKind, NodeSpec},
    picture::{
      PaintOp, Picture,
      kurbo::{Affine, Shape, Stroke},
    },
    record::Record,
    report::Size,
    tree,
  },
};

use crate::{
  Active,
  access::Access,
  view::{Cx, Node, NodeHandle, View, finish, handle, properties},
};

/// How finely a curved shape is flattened into the path recorded, in pixels.
const TOLERANCE: f64 = 0.1;

/// What a [`Painted`] box's closure draws on: its size and its states, and a recording of
/// kurbo's shapes and peniko's brushes in pixels, its origin the box's top leading corner,
/// which the platform replays in the scene's order at the device's scale.
pub struct Canvas<'a> {
  picture: &'a mut Picture,
  width: f32,
  height: f32,
  scale: f64,
  states: &'a dyn Fn() -> StateSet,
}

impl Canvas<'_> {
  /// The box's width, in pixels.
  #[must_use]
  pub const fn width(&self) -> f32 {
    self.width
  }

  /// The box's height, in pixels.
  #[must_use]
  pub const fn height(&self) -> f32 {
    self.height
  }

  /// Device pixels per pixel.
  #[must_use]
  pub const fn scale(&self) -> f64 {
    self.scale
  }

  /// The box's states, its enclosing control's included (a checkbox's mark reads `Checked`):
  /// reading them records the picture again when they change.
  #[must_use]
  pub fn states(&self) -> StateSet {
    (self.states)()
  }

  /// Fills `shape` with `paint`: a color, a brush, or the box's ink ([`Paint::Ink`]).
  pub fn fill(&mut self, shape: &impl Shape, paint: impl Into<Paint>) {
    self.picture.push(PaintOp::Fill {
      path: shape.to_path(TOLERANCE),
      paint: paint.into(),
    });
  }

  /// Strokes `shape`'s outline `width` pixels wide with `paint`.
  pub fn stroke(&mut self, shape: &impl Shape, width: f64, paint: impl Into<Paint>) {
    self.picture.push(PaintOp::Stroke {
      path: shape.to_path(TOLERANCE),
      stroke: Stroke::new(width),
      paint: paint.into(),
    });
  }

  /// Cuts what is drawn next to `shape`, until the matching [`Canvas::pop`].
  pub fn push_clip(&mut self, shape: &impl Shape) {
    self
      .picture
      .push(PaintOp::PushClip(shape.to_path(TOLERANCE)));
  }

  /// Draws what is drawn next at `opacity` (0 to 1), until the matching [`Canvas::pop`].
  pub fn push_opacity(&mut self, opacity: f32) {
    self
      .picture
      .push(PaintOp::PushOpacity(opacity.clamp(0., 1.)));
  }

  /// Ends the innermost [`Canvas::push_clip`] or [`Canvas::push_opacity`].
  pub fn pop(&mut self) {
    self.picture.push(PaintOp::Pop);
  }

  /// Draws what is drawn next under `transform`, from the box's own pixels.
  pub fn set_transform(&mut self, transform: Affine) {
    self.picture.push(PaintOp::Transform(transform));
  }
}

/// A box the app draws on a [`Canvas`]: kurbo's shapes and peniko's brushes, recorded and
/// replayed by the platform in the scene's order, beneath what is drawn after it.
///
/// The closure runs again whenever a signal it reads changes, whenever the states it read
/// change, and whenever the box is laid out at another size. The box is as large as its
/// sizing makes it: give it a width and a height, or a weight in its layout. Without a
/// description it is decoration a reader skips.
///
/// ```
/// use skene_gui::{painted::shapes::Circle, prelude::*};
///
/// let level = Signal::new(0.5_f32);
///
/// Painted::new(move |canvas| {
///   let radius = f64::from(canvas.height() / 2.);
///
///   canvas.fill(&Circle::new((radius, radius), radius), Color::rgb(0x34c759));
///   canvas.fill(
///     &Circle::new((radius, radius), radius * f64::from(level.get())),
///     Paint::Ink,
///   );
/// })
/// .set_width(48)
/// .set_height(48)
/// .set_description("Level");
/// ```
#[must_use]
pub struct Painted {
  paint: Rc<dyn Fn(&mut Canvas<'_>)>,
  record: Record,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Painted {
  /// A box drawn by `paint`.
  pub fn new(paint: impl Fn(&mut Canvas<'_>) + 'static) -> Self {
    Self {
      paint: Rc::new(paint),
      record: Record::new(),
      handle: None,
      access: Access::default(),
    }
  }

  properties! {
    /// The width.
    set_width, derive_width: WIDTH, Dimension;
    /// The height.
    set_height, derive_height: HEIGHT, Dimension;
    /// The least width.
    set_min_width, derive_min_width: MIN_WIDTH, Dimension;
    /// The least height.
    set_min_height, derive_min_height: MIN_HEIGHT, Dimension;
    /// The greatest width.
    set_max_width, derive_max_width: MAX_WIDTH, Dimension;
    /// The greatest height.
    set_max_height, derive_max_height: MAX_HEIGHT, Dimension;
    /// The ink [`Paint::Ink`] draws in, inherited where unset.
    set_color, derive_color: COLOR, Color;
  }

  handle!();

  /// Describes the picture for a reader.
  pub fn set_description(self, description: impl Into<Rc<str>>) -> Self {
    let description = description.into();

    self.derive_description(move || Rc::clone(&description))
  }

  /// Describes the picture for a reader, kept current.
  pub fn derive_description<S: Into<Rc<str>>>(
    mut self,
    description: impl Fn() -> S + 'static,
  ) -> Self {
    self.access.name = Some(Rc::new(move || description().into()));

    self
  }
}

impl View for Painted {
  fn build(self, cx: &mut Cx) -> Node {
    let described = self.access.name.is_some();
    let mut spec = NodeSpec::new(NodeKind::Painted);

    if !described {
      spec = spec.with_role(Role::Presentation);
    }

    let id = cx.create(spec);
    let size = Signal::new(None::<Size>);
    let laid = Memo::new(move || {
      size
        .get()
        .map(|size| (size.width.to_bits(), size.height.to_bits()))
    });
    let paint = self.paint;

    finish(id, self.record, cx, self.handle.as_ref());
    tree::handle_size::<Active>(id, move |laid_out| size.set(Some(laid_out)));
    tree::bind::<Active, _>(id, contents::PICTURE, move || {
      let mut picture = Picture::new();

      if let Some((width, height)) = laid.get() {
        let scale = skene_foundation::untrack(|| size.get().map_or(1., |size| size.scale));

        paint(&mut Canvas {
          picture: &mut picture,
          width: f32::from_bits(width),
          height: f32::from_bits(height),
          scale,
          states: &|| tree::resolved_states::<Active>(id),
        });
      }

      Rc::new(picture)
    });

    self.access.apply(id);

    Node(id)
  }
}
