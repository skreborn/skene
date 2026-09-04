//! A ready-made slider in the Simple design language.

use std::{marker::PhantomData, rc::Rc};

use skene_core::{
  Alignment,
  Dimension,
  ItemAlignment,
  MouseCursor,
  SliderRange,
  next::{AnyKind, Driver, HasChildren, NodeId, Part, State, View, When},
};
use skene_reactive::{Get, Set, Signal};
use skene_views::{Container, CornerRadius, Range, RawSlider, Stack};

use super::{
  SimplePalette,
  shape::{ROUND, THUMB_SIZE, TRACK_HEIGHT, TRACK_RADIUS, as_part, border, define_defaults},
};

/// A ready-made slider: a track on the secondary surface, the run filled to the value
/// in the primary surface, and a thumb that rings while focused.
///
/// The track, the fill and the thumb are the control's [`Part::Track`], [`Part::Fill`]
/// and [`Part::Thumb`]; the fill's width and the thumb's place derive from the value.
/// The value flows both ways. For a slider drawn some other way, compose [`RawSlider`]
/// directly.
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
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<D>();

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
        .set_height(TRACK_HEIGHT)
        .set_corner_radius(TRACK_RADIUS)
        .set_color(SimplePalette::<D::Scalar>::PRIMARY_SURFACE),
    );
    let track = as_part::<D>(
      Part::Track,
      Container::<D>::new()
        .set_width(Dimension::Fraction(1.))
        .set_height(TRACK_HEIGHT)
        .set_corner_radius(TRACK_RADIUS)
        .set_color(SimplePalette::<D::Scalar>::SECONDARY_SURFACE)
        .set_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
        .set_child(fill),
    );
    let thumb = as_part::<D>(
      Part::Thumb,
      Container::<D>::new()
        .set_width(THUMB_SIZE)
        .set_height(THUMB_SIZE)
        .set_corner_radius(ROUND)
        .set_color(SimplePalette::<D::Scalar>::PRIMARY_SURFACE)
        .set_border(border(2., SimplePalette::<D::Scalar>::BACKGROUND).when(
          State::FocusVisible,
          border::<D::Scalar>(2., SimplePalette::<D::Scalar>::RING),
        )),
    );
    let rail = Container::<D>::new()
      .derive_width(move || Dimension::Fraction(range.fraction_of(placed())))
      .set_height(THUMB_SIZE)
      .set_alignment(Alignment::new(ItemAlignment::End, ItemAlignment::Center))
      .set_child(thumb);
    let mut behavior = Range::new()
      .set_min(min)
      .set_max(max)
      .set_step(step)
      .derive_value(move || value());

    if let Some(action) = on_change {
      behavior = behavior.handle_change(move |value| action(value));
    }

    RawSlider::new(behavior)
      .set_width(Dimension::Fraction(1.))
      .set_cursor(MouseCursor::Click)
      .set_child(
        Container::<D>::new()
          .set_width(Dimension::Fraction(1.))
          .set_height(THUMB_SIZE)
          .set_child(Stack::new().set_children((track, rail))),
      )
      .build()
  }
}
