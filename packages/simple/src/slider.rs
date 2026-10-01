//! A ready-made slider in the Simple design language.

use std::{marker::PhantomData, rc::Rc};

use skene_foundation::{
  Alignment,
  AnyKind,
  BasicSlider,
  Container,
  ContainerBase,
  CornerRadius,
  DecorationBase,
  DecorationWithCornerRadius,
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
  StackBase,
  State,
  View,
  Wraps,
  realize::Source,
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
#[doc(hidden)]
#[must_use]
pub struct SimpleSlider<P: Platform> {
  label: Option<String>,
  range: Source<SliderRange>,
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
      range: Source::Fixed(SliderRange::new(0., 100., 1.)),
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

  /// Sets the range: its bounds and the step values snap to.
  #[inline(always)]
  pub fn set_range(mut self, range: SliderRange) -> Self {
    self.range = Source::Fixed(range);

    self
  }

  /// Derives the range from the signals `range` reads.
  pub fn derive_range(mut self, range: impl Fn() -> SliderRange + 'static) -> Self {
    self.range = Source::derive(range);

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
      range,
      value,
      on_change,
      ..
    } = self;
    let range = range.into_read();
    let value: Rc<dyn Fn() -> f32> = value.unwrap_or_else(|| {
      let range = Rc::clone(&range);

      Rc::new(move || range().min())
    });
    let filled = Rc::clone(&value);
    let placed = Rc::clone(&value);
    let fill = as_part::<P>(
      Part::Fill,
      Container::<P>::new()
        .derive_width({
          let range = Rc::clone(&range);

          move || Dimension::Fraction(range().fraction_of(filled()))
        })
        .set_height(Dimension::Length(shape.track_height))
        .set_corner_radius(shape.track_height.to_f32() / 2.)
        .set_background(SimplePalette::<P::Scalar>::PRIMARY_SURFACE),
    );
    let track = as_part::<P>(
      Part::Track,
      Container::<P>::new()
        .set_width(Dimension::Fraction(1.))
        .set_height(Dimension::Length(shape.track_height))
        .set_corner_radius(shape.track_height.to_f32() / 2.)
        .set_background(SimplePalette::<P::Scalar>::SECONDARY_SURFACE),
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
          end: shape.thumb_size,
          bottom: P::Scalar::default(),
          start: P::Scalar::default(),
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
            .derive_edges({
              let range = Rc::clone(&range);

              move || Inset {
                top: Dimension::Length(P::Scalar::default()),
                end: Dimension::Auto,
                bottom: Dimension::Auto,
                start: Dimension::Fraction(range().fraction_of(placed())),
              }
            })
            .set_child(thumb),
        ),
      );
    let mut behavior = Slider::new()
      .derive_range(move || range())
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
              .set_item_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
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
    .set_background(
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

/// What a platform's `SimpleSlider` carries.
pub trait SimpleSliderBase<P: Platform>: Wraps<Inner = SimpleSlider<P>> {
  /// A slider over `0..=100` stepping by `1`, holding its minimum.
  #[must_use]
  fn new() -> Self {
    Self::wrap(SimpleSlider::new())
  }

  /// Sets the accessible name; nothing visible names a slider.
  #[must_use]
  fn set_label(self, label: impl Into<String>) -> Self {
    Self::wrap(self.into_inner().set_label(label))
  }

  /// Sets the range: its bounds and the step values snap to.
  #[must_use]
  fn set_range(self, range: SliderRange) -> Self {
    Self::wrap(self.into_inner().set_range(range))
  }

  /// Derives the range from the signals `range` reads.
  #[must_use]
  fn derive_range(self, range: impl Fn() -> SliderRange + 'static) -> Self {
    Self::wrap(self.into_inner().derive_range(range))
  }

  /// Sets the value.
  #[must_use]
  fn set_value(self, value: f32) -> Self {
    Self::wrap(self.into_inner().set_value(value))
  }

  /// Derives the value from the signals `value` reads.
  #[must_use]
  fn derive_value(self, value: impl Fn() -> f32 + 'static) -> Self {
    Self::wrap(self.into_inner().derive_value(value))
  }

  /// Runs `action` with the value a move asks for.
  #[must_use]
  fn handle_change(self, action: impl Fn(f32) + 'static) -> Self {
    Self::wrap(self.into_inner().handle_change(action))
  }

  /// Binds the value to `signal` in both directions.
  #[must_use]
  fn bind_value(self, signal: Signal<f32>) -> Self {
    Self::wrap(self.into_inner().bind_value(signal))
  }
}
