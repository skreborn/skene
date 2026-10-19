//! A box the app draws: a recording of kurbo's shapes and peniko's brushes in device pixels,
//! which the platform replays in the scene's order, recorded again whenever what its closure
//! read changes or the box is drawn at another device size or scale.

use std::{marker::PhantomData, rc::Rc};

/// The geometry a [`Canvas`] draws with (kurbo).
pub use crate::seam::picture::kurbo as shapes;
/// The paints a [`Canvas`] draws with (peniko), beside the node's own ink ([`Paint::Ink`]).
pub use crate::seam::picture::peniko as paints;
pub use crate::seam::picture::{Paint, PaintOp, Picture};
use crate::{
  Color, Dimension, Memo, Signal, StateSet,
  seam::{
    Realization,
    access::Role,
    content::contents,
    node::{NodeKind, NodeSpec},
    picture::kurbo::{Affine, Shape, Size, Stroke, Vec2},
    record::Record,
    report, tree,
  },
  view::{
    Access, AnyKind, Child, Cx, HasRecord, Node, NodeHandle, View, Wraps, trait_properties,
    wraps_itself,
  },
};

/// How finely a curved shape is flattened into the path recorded, in device pixels.
const TOLERANCE: f64 = 0.1;

/// What a [`Painted`] box's closure draws on: its size in device pixels, its scale, its
/// states, and a recording of kurbo's shapes and peniko's brushes in device pixels, its origin
/// the box's top leading corner on the device's pixel grid, which the platform replays in the
/// scene's order.
///
/// A line one device pixel wide at `x = 0.5` covers exactly one column of the device's pixels
/// at any scale. To draw in the box's layout units instead, set the transform to
/// [`Canvas::to_device`] and measure by [`Canvas::layout_size`].
pub struct Canvas<'a> {
  picture: &'a mut Picture,
  size: Size,
  scale: Vec2,
  layout: Size,
  states: &'a dyn Fn() -> StateSet,
}

impl Canvas<'_> {
  /// The box's size in device pixels, its edges on the device's pixel grid: whole numbers.
  #[must_use]
  pub const fn size(&self) -> Size {
    self.size
  }

  /// Device pixels per layout unit along each axis: the window's scale on a pixel platform.
  #[must_use]
  pub const fn scale(&self) -> Vec2 {
    self.scale
  }

  /// The transform from the box's layout units to its device pixels.
  #[must_use]
  pub fn to_device(&self) -> Affine {
    Affine::scale_non_uniform(self.scale.x, self.scale.y)
  }

  /// The box's size in layout units, as laid out.
  #[must_use]
  pub const fn layout_size(&self) -> Size {
    self.layout
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

  /// Strokes `shape`'s outline as `stroke` draws it (its width, joins, caps and dashes) with
  /// `paint`.
  pub fn stroke(&mut self, shape: &impl Shape, stroke: &Stroke, paint: impl Into<Paint>) {
    self.picture.push(PaintOp::Stroke {
      path: shape.to_path(TOLERANCE),
      stroke: stroke.clone(),
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

  /// Draws what is drawn next under `transform`, from the box's device pixels.
  pub fn set_transform(&mut self, transform: Affine) {
    self.picture.push(PaintOp::Transform(transform));
  }
}

/// Whether the platform paints a [`Painted`] box: the pixels' own fact, which a box's
/// fallback ([`PaintedBase::set_fallback_builder`]) stands in for until it holds.
pub struct Painting<R: Realization>(PhantomData<fn() -> R>);

impl<R: Realization> Painting<R> {
  /// Whether painting is available now, subscribing the running effect: from the start, and
  /// for good, on every platform Skene draws (Linux's windows among them).
  #[must_use]
  pub fn available() -> bool {
    R::painting_available()
  }
}

/// A box the app draws on a [`Canvas`]: kurbo's shapes and peniko's brushes, recorded in
/// device pixels and replayed by the platform in the scene's order, beneath what is drawn
/// after it.
///
/// The closure runs again whenever a signal it reads changes, whenever the states it read
/// change, and whenever the box is drawn at another device size or scale (laid out at another
/// size, moved to a window or a popup of another scale). The box is as large as its sizing
/// makes it: give it a width and a height, or a weight in its layout. Without a description it
/// is decoration a reader skips.
#[must_use]
pub struct Painted<R: Realization> {
  paint: Rc<dyn Fn(&mut Canvas<'_>)>,
  record: Record,
  handle: Option<NodeHandle>,
  access: Access,
  fallback: Option<Child<R>>,
}

wraps_itself!(Painted<R> where R: Realization);

impl<R: Realization> HasRecord for Painted<R> {
  #[inline(always)]
  fn record_mut(&mut self) -> &mut Record {
    &mut self.record
  }
}

/// What a [`Painted`] box carries: what draws it, its fallback, its size, its ink, its
/// handle and its description.
pub trait PaintedBase<R: Realization>: Wraps<Inner = Painted<R>> {
  /// A box drawn by `paint`.
  fn new(paint: impl Fn(&mut Canvas<'_>) + 'static) -> Self {
    Self::wrap(Painted {
      paint: Rc::new(paint),
      record: Record::new(),
      handle: None,
      access: Access::default(),
      fallback: None,
    })
  }

  /// What shows in the box's place while painting is not available ([`Painting::available`]):
  /// the view `fallback` builds, built at most once; on a platform that paints from the start
  /// it is never built.
  fn set_fallback_builder<V: View<R>>(mut self, fallback: impl FnOnce() -> V + 'static) -> Self {
    self.inner_mut().fallback = Some(Child::new(move |cx: &mut Cx<R>| fallback().build(cx)));

    self
  }

  trait_properties! {
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

  /// Writes the box's node into `handle` once built.
  fn set_handle(mut self, handle: &NodeHandle) -> Self {
    self.inner_mut().handle = Some(*handle);

    self
  }

  /// Describes the picture for a reader.
  fn set_description(self, description: impl Into<Rc<str>>) -> Self {
    let description = description.into();

    self.derive_description(move || Rc::clone(&description))
  }

  /// Describes the picture for a reader, kept current.
  fn derive_description<S: Into<Rc<str>>>(mut self, description: impl Fn() -> S + 'static) -> Self {
    self.inner_mut().access.name = Some(Rc::new(move || description().into()));

    self
  }
}

impl<R: Realization> PaintedBase<R> for Painted<R> {}

impl<R: Realization> AnyKind for Painted<R> {}

impl<R: Realization> View<R> for Painted<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    if !Painting::<R>::available()
      && let Some(fallback) = self.fallback
    {
      return fallback.build(cx);
    }

    let described = self.access.name.is_some();
    let mut spec = NodeSpec::new(NodeKind::Painted);

    if !described {
      spec = spec.with_role(Role::Presentation);
    }

    let id = cx.create(spec);
    let size = Signal::new(None::<report::Size>);
    // What the picture is recorded at: the device size and the scale, a move that keeps
    // both replaying the same recording.
    let drawn = Memo::new(move || {
      size.get().map(|size| {
        (
          size.device,
          size.scale.to_bits(),
          size.width.to_bits(),
          size.height.to_bits(),
        )
      })
    });
    let paint = self.paint;

    cx.finish(id, self.record, self.handle.as_ref());
    tree::handle_size::<R>(id, move |laid_out| size.set(Some(laid_out)));
    tree::bind::<R, _>(id, contents::PICTURE, move || {
      let mut picture = Picture::new();

      if let Some(((device_width, device_height), scale, width, height)) = drawn.get() {
        let scale = f64::from_bits(scale);

        paint(&mut Canvas {
          picture: &mut picture,
          size: Size::new(f64::from(device_width), f64::from(device_height)),
          scale: Vec2::new(scale, scale),
          layout: Size::new(
            f64::from(f32::from_bits(width)),
            f64::from(f32::from_bits(height)),
          ),
          states: &|| tree::resolved_states::<R>(id),
        });
      }

      Rc::new(picture)
    });

    self.access.apply::<R>(id);

    Node::new(id)
  }
}
