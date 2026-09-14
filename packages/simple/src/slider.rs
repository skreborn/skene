//! A ready-made slider in the Simple design language.

use std::{marker::PhantomData, rc::Rc};

use skene_core::{
  Alignment,
  AnyKind,
  Dimension,
  Driver,
  HasChildren,
  Inset,
  ItemAlignment,
  MouseCursor,
  NodeId,
  Part,
  Scene,
  SliderRange,
  State,
  View,
  When,
};
use skene_reactive::{Get, Set, Signal};
use skene_views::{Anchor, Container, CornerRadius, Pinned, Range, RawSlider, Stack};

use super::{
  SimplePalette,
  shape::{ROUND, SimpleShape, SimpleUnit, TRACK_RADIUS, as_part, border, define_defaults, shape},
};

/// A ready-made slider: a track on the secondary surface, the run filled to the value
/// in the primary surface, and a thumb that rings while focused, or in cells takes the
/// ring's color.
///
/// The track, the fill and the thumb are the control's [`Part::Track`], [`Part::Fill`]
/// and [`Part::Thumb`]; the fill runs to the thumb's leading edge and the thumb travels
/// the track less its own size, so both stay inside the track at either end. The value
/// flows both ways. For a slider drawn some other way, compose [`RawSlider`] directly.
#[must_use]
pub struct SimpleSlider<D: Driver> {
  min: f32,
  max: f32,
  step: f32,
  value: Option<Rc<dyn Fn() -> f32>>,
  on_change: Option<Rc<dyn Fn(f32)>>,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> SimpleSlider<D> {
  /// A slider over `0..=100` stepping by `1`, holding its minimum.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      min: 0.,
      max: 100.,
      step: 1.,
      value: None,
      on_change: None,
      driver: PhantomData,
    }
  }

  /// Sets the lowest value.
  #[inline(always)]
  pub const fn set_min(mut self, min: f32) -> Self {
    self.min = min;

    self
  }

  /// Sets the highest value.
  #[inline(always)]
  pub const fn set_max(mut self, max: f32) -> Self {
    self.max = max;

    self
  }

  /// Sets the step a key moves by and values snap to; non-positive for a continuous
  /// range.
  #[inline(always)]
  pub const fn set_step(mut self, step: f32) -> Self {
    self.step = step;

    self
  }

  /// Sets the value.
  #[inline(always)]
  pub fn set_value(mut self, value: f32) -> Self {
    self.value = Some(Rc::new(move || value));

    self
  }

  /// Derives the value from the signals `value` reads.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> f32 + 'static) -> Self {
    self.value = Some(Rc::new(value));

    self
  }

  /// Runs `action` with the value a move asks for.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(f32) + 'static) -> Self {
    self.on_change = Some(Rc::new(action));

    self
  }

  /// Binds the value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_value(self, signal: Signal<f32>) -> Self {
    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value))
  }
}

impl<D: Driver> Default for SimpleSlider<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> AnyKind for SimpleSlider<D> {}

impl<D: Driver> View<D> for SimpleSlider<D>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<D>();

    let shape = shape::<D>();
    let Self {
      min,
      max,
      step,
      value,
      on_change,
      ..
    } = self;
    let range = SliderRange::new(min, max, step);
    let value: Rc<dyn Fn() -> f32> = value.unwrap_or_else(|| Rc::new(move || min));
    let filled = Rc::clone(&value);
    let placed = Rc::clone(&value);
    let fill = as_part::<D>(
      Part::Fill,
      Container::<D>::new()
        .derive_width(move || Dimension::Fraction(range.fraction_of(filled())))
        .set_height(Dimension::Length(shape.track_height))
        .set_corner_radius(TRACK_RADIUS)
        .set_color(SimplePalette::<D::Scalar>::PRIMARY_SURFACE),
    );
    let track = as_part::<D>(
      Part::Track,
      Container::<D>::new()
        .set_width(Dimension::Fraction(1.))
        .set_height(Dimension::Length(shape.track_height))
        .set_corner_radius(TRACK_RADIUS)
        .set_color(SimplePalette::<D::Scalar>::SECONDARY_SURFACE),
    );
    // The thumb is pinned, so the stack builds it; its part is recorded once it is.
    let thumb_anchor = Anchor::new();
    let thumb = thumb::<D>(shape, &thumb_anchor);
    // Each lane spares the thumb's width at its end, so the fill's fraction and the
    // thumb's travel both span the track less the thumb.
    let lane = || {
      Container::<D>::new()
        .set_width(Dimension::Fraction(1.))
        .set_height(Dimension::Length(shape.thumb_size))
        .set_padding(Inset {
          top: D::Scalar::default(),
          right: shape.thumb_size,
          bottom: D::Scalar::default(),
          left: D::Scalar::default(),
        })
        .set_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
    };
    let filled_lane = lane().set_child(fill);
    let rail_lane = lane()
      .set_alignment(Alignment::new(
        ItemAlignment::Stretch,
        ItemAlignment::Stretch,
      ))
      .set_child(
        Stack::new().set_child(
          Pinned::new()
            .derive_edges(move || Inset {
              top: Dimension::Length(D::Scalar::default()),
              right: Dimension::Auto,
              bottom: Dimension::Auto,
              left: Dimension::Fraction(range.fraction_of(placed())),
            })
            .set_child(thumb),
        ),
      );
    let mut behavior = Range::new()
      .set_min(min)
      .set_max(max)
      .set_step(step)
      .derive_value(move || value());

    if let Some(action) = on_change {
      behavior = behavior.handle_change(move |value| action(value));
    }

    let node = RawSlider::new(behavior)
      .set_width(Dimension::Fraction(1.))
      .set_cursor(MouseCursor::Click)
      .set_child(
        Container::<D>::new()
          .set_width(Dimension::Fraction(1.))
          .set_height(Dimension::Length(shape.thumb_size))
          .set_child(
            Stack::new()
              .set_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
              .set_children((track, filled_lane, rail_lane)),
          ),
      )
      .build();

    if let Some(thumb) = thumb_anchor.node() {
      Scene::<D>::with(|scene| scene.set_part(thumb, Part::Thumb));
    }

    node
  }
}

/// The thumb: a box in the unit's thumb surface, ringed while focus is visible where
/// the ring has thickness and recolored where it has none, bound to `anchor`.
fn thumb<D: Driver>(shape: SimpleShape<D::Scalar>, anchor: &Anchor) -> Container<D>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  Container::<D>::new()
    .bind_anchor(anchor)
    .set_width(Dimension::Length(shape.thumb_size))
    .set_height(Dimension::Length(shape.thumb_size))
    .set_corner_radius(ROUND)
    .set_color(
      shape
        .thumb_surface
        .when(State::FocusVisible, shape.thumb_focus),
    )
    .set_border(
      border(shape.thumb_ring, SimplePalette::<D::Scalar>::BACKGROUND).when(
        State::FocusVisible,
        border(shape.thumb_ring, SimplePalette::<D::Scalar>::RING),
      ),
    )
}
