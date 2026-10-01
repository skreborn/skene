//! A ready-made slider in the Simple design language.

use std::{marker::PhantomData, rc::Rc};

use skene_foundation::{
  Alignment,
  AnyKind,
  BasicSlider,
  Container,
  CornerRadius,
  Dimension,
  HasChild,
  HasChildren,
  Inset,
  ItemAlignment,
  NodeHandle,
  NodeId,
  Part,
  Pinned,
  Platform,
  Scalar,
  Scene,
  Slider,
  SliderRange,
  Stack,
  State,
  View,
};
use skene_reactive::{Get, Set, Signal, untrack};

use super::{
  SimplePalette,
  shape::{SimpleShape, SimpleUnit, as_part, click_cursor, define_defaults, shape},
};

/// A ready-made slider: a track on the secondary surface, the run filled to the value
/// in the primary surface, and a thumb that rings while focused, or in cells takes the
/// ring's color.
///
/// The track, the fill and the thumb are the control's [`Part::Track`], [`Part::Fill`]
/// and [`Part::Thumb`]; the fill runs to the thumb's leading edge and the thumb travels
/// the track less its own size, so both stay inside the track at either end. The value
/// flows both ways. For a slider drawn some other way, compose [`BasicSlider`] directly.
#[must_use]
pub struct SimpleSlider<P: Platform> {
  label: Option<String>,
  min: f32,
  max: f32,
  step: f32,
  value: Option<Rc<dyn Fn() -> f32>>,
  on_change: Option<Rc<dyn Fn(f32)>>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> std::fmt::Debug for SimpleSlider<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("SimpleSlider")
      .finish_non_exhaustive()
  }
}

impl<P: Platform> SimpleSlider<P> {
  /// A slider over `0..=100` stepping by `1`, holding its minimum.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      label: None,
      min: 0.,
      max: 100.,
      step: 1.,
      value: None,
      on_change: None,
      platform: PhantomData,
    }
  }

  /// Sets the accessible name; nothing visible names a slider.
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
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
  pub fn derive_value(mut self, value: impl Fn() -> f32 + 'static) -> Self {
    self.value = Some(Rc::new(value));

    self
  }

  /// Runs `action` with the value a move asks for.
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

impl<P: Platform> Default for SimpleSlider<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> AnyKind for SimpleSlider<P> {}

impl<P: Platform> View<P> for SimpleSlider<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<P>();

    let shape = shape::<P>();
    let Self {
      label,
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
    let fill = as_part::<P>(
      Part::Fill,
      Container::<P>::new()
        .derive_width(move || Dimension::Fraction(range.fraction_of(filled())))
        .set_height(Dimension::Length(shape.track_height))
        .set_corner_radius(shape.track_height.to_f32() / 2.)
        .set_color(SimplePalette::<P::Scalar>::PRIMARY_SURFACE),
    );
    let track = as_part::<P>(
      Part::Track,
      Container::<P>::new()
        .set_width(Dimension::Fraction(1.))
        .set_height(Dimension::Length(shape.track_height))
        .set_corner_radius(shape.track_height.to_f32() / 2.)
        .set_color(SimplePalette::<P::Scalar>::SECONDARY_SURFACE),
    );
    // The thumb is pinned, so the stack builds it; its part is recorded once it is.
    let thumb_handle = NodeHandle::<P>::new();
    let thumb = thumb::<P>(shape, thumb_handle);
    // Each lane spares the thumb's width at its end, so the fill's fraction and the
    // thumb's travel both span the track less the thumb.
    let lane = || {
      Container::<P>::new()
        .set_width(Dimension::Fraction(1.))
        .set_height(Dimension::Length(shape.thumb_size))
        .set_padding(Inset {
          top: P::Scalar::default(),
          right: shape.thumb_size,
          bottom: P::Scalar::default(),
          left: P::Scalar::default(),
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
        Stack::new().add_child(
          Pinned::new()
            .derive_edges(move || Inset {
              top: Dimension::Length(P::Scalar::default()),
              right: Dimension::Auto,
              bottom: Dimension::Auto,
              left: Dimension::Fraction(range.fraction_of(placed())),
            })
            .set_child(thumb),
        ),
      );
    let mut behavior = Slider::new()
      .set_min(min)
      .set_max(max)
      .set_step(step)
      .set_thumb_extent(shape.thumb_size.to_f32())
      .derive_value(move || value());

    if let Some(action) = on_change {
      behavior = behavior.handle_change(move |value| action(value));
    }

    let mut slider = BasicSlider::new(behavior);

    if let Some(label) = label {
      slider = slider.set_accessible_name(label);
    }

    let node = slider
      .set_width(Dimension::Fraction(1.))
      .set_cursor(click_cursor())
      .set_child(
        Container::<P>::new()
          .set_width(Dimension::Fraction(1.))
          .set_height(Dimension::Length(shape.thumb_size))
          .set_child(
            Stack::new()
              .set_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
              .set_children((track, filled_lane, rail_lane)),
          ),
      )
      .build();

    if let Some(thumb) = untrack(|| thumb_handle.node()) {
      Scene::<P>::with(|scene| scene.set_part(thumb, Part::Thumb));
    }

    node
  }
}

/// The thumb: a box in the unit's thumb surface, ringed while focus is visible where
/// the ring has thickness and recolored where it has none, bound to `handle`.
fn thumb<P: Platform>(shape: SimpleShape<P::Scalar>, handle: NodeHandle<P>) -> Container<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  Container::<P>::new()
    .bind_handle(handle)
    .set_width(Dimension::Length(shape.thumb_size))
    .set_height(Dimension::Length(shape.thumb_size))
    .set_corner_radius(shape.thumb_size.to_f32() / 2.)
    .set_color(
      shape
        .thumb_surface
        .when(State::FocusVisible, shape.thumb_focus),
    )
    .set_border_width(Inset::all(shape.thumb_ring))
    .set_border_color(
      SimplePalette::<P::Scalar>::BACKGROUND
        .when(State::FocusVisible, SimplePalette::<P::Scalar>::RING),
    )
}
