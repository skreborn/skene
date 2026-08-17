//! A ready-made slider in the Simple design language.

use std::rc::Rc;

use skene_core::{
  Alignment,
  Border,
  Condition,
  Dimension,
  HasChildren,
  ItemAlignment,
  SliderRange,
  Source,
  View,
};

use crate::{Container, Enhanced, HasDecoration, RawSlider, SimplePalette, Stack, WebSlider};

/// The track's thickness.
const TRACK_HEIGHT: f32 = 6.;
/// The thumb's diameter.
const THUMB_SIZE: f32 = 16.;

/// A ready-made slider in the Simple design language.
///
/// Drawn by the framework once the client owns the page: the track, the filled run, and
/// the thumb are ordinary views, so the fill follows the value with no vendor
/// pseudo-element and no script-bound custom property—the two things that make a native
/// range hard to style. Without script the browser's own range stands in, which works on
/// its own.
///
/// The value flows both ways. For a slider drawn some other way, compose [`RawSlider`]
/// directly.
#[must_use]
pub struct SimpleSlider {
  min: f32,
  max: f32,
  step: f32,
  value: Option<Source<f32>>,
  change_handler: Option<Rc<dyn Fn(f32)>>,
}

impl SimpleSlider {
  /// Creates a slider selecting within `min..=max`.
  #[inline(always)]
  pub fn new(min: f32, max: f32) -> Self {
    Self {
      min,
      max,
      step: 1.,
      value: None,
      change_handler: None,
    }
  }

  /// Sets the step between selectable values.
  #[inline(always)]
  pub fn set_step(mut self, step: f32) -> Self {
    self.step = step;

    self
  }

  /// Sets a static value.
  #[inline(always)]
  pub fn set_value(mut self, value: f32) -> Self {
    self.value = Some(Source::Static(value));

    self
  }

  /// Sets a derived value, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> f32 + 'static) -> Self {
    self.value = Some(Source::Derived(Rc::new(value)));

    self
  }

  /// Sets the change handler, run with the new value on every move.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(f32) + 'static) -> Self {
    self.change_handler = Some(Rc::new(action));

    self
  }

  /// Binds the value to `signal` in both directions: the slider shows the signal and
  /// each move writes it back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::Signal<f32>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value))
  }
}

impl View for SimpleSlider {
  fn build(self) -> skene_core::NodeId {
    let Self {
      min,
      max,
      step,
      value,
      change_handler,
    } = self;
    let range = SliderRange::new(min, max, step);
    let reads = move || value.as_ref().map_or(min, Source::get);

    Enhanced::new({
      let reads = reads.clone();
      let change_handler = change_handler.clone();

      move || native(min, max, step, reads.clone(), change_handler.clone())
    })
    .set_child(move || drawn(range, reads.clone(), change_handler.clone()))
    .build()
  }
}

/// The scriptless form: the browser's own range, which needs no script to work.
fn native(
  min: f32,
  max: f32,
  step: f32,
  value: impl Fn() -> f32 + 'static,
  change_handler: Option<Rc<dyn Fn(f32)>>,
) -> WebSlider {
  // The same footprint as the drawn track and thumb, so the swap shifts nothing.
  let mut control = WebSlider::new(min, max)
    .set_step(step)
    .derive_value(value)
    .set_width(Dimension::Fraction(1.))
    .set_height(THUMB_SIZE);

  if let Some(action) = change_handler {
    control = control.handle_change(move |value| action(value));
  }

  control
}

/// The drawn form: a track, the run filled to the value, and a thumb over it.
fn drawn(
  range: SliderRange,
  value: impl Fn() -> f32 + Clone + 'static,
  change_handler: Option<Rc<dyn Fn(f32)>>,
) -> impl View {
  let filled = value.clone();
  let placed = value.clone();
  let palette = SimplePalette::current();
  let track = Container::new()
    .set_width(Dimension::Fraction(1.))
    .set_height(TRACK_HEIGHT)
    .set_border_radius(TRACK_HEIGHT / 2.)
    .set_color(palette.secondary.surface.clone())
    .set_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
    .set_child(
      Container::new()
        .derive_width(move || Dimension::Fraction(range.fraction_of(filled())))
        .set_height(TRACK_HEIGHT)
        .set_border_radius(TRACK_HEIGHT / 2.)
        .set_color(palette.primary.surface.clone()),
    );
  // The thumb rides the fill's trailing edge, so it needs the same fraction rather than
  // a position of its own.
  let thumb = Container::new()
    .derive_width(move || Dimension::Fraction(range.fraction_of(placed())))
    .set_height(THUMB_SIZE)
    .set_alignment(Alignment::new(ItemAlignment::End, ItemAlignment::Center))
    .set_child(
      Container::new()
        .set_width(THUMB_SIZE)
        .set_height(THUMB_SIZE)
        .set_border_radius(THUMB_SIZE / 2.)
        .set_color(palette.primary.surface)
        .set_border(palette.background.map(|color| Border::new(2., color)))
        .set_border_when(
          Condition::Focused,
          palette.ring.map(|color| Border::new(2., color)),
        ),
    );
  let mut control = RawSlider::new(range.min(), range.max())
    .set_step(range.step())
    .derive_value(value);

  if let Some(action) = change_handler {
    control = control.handle_change(move |value| action(value));
  }

  control.set_child(
    Container::new()
      .set_width(Dimension::Fraction(1.))
      .set_height(THUMB_SIZE)
      .set_child(Stack::new().set_children((track, thumb))),
  )
}
