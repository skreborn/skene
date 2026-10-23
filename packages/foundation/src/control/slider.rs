//! A slider's behavior.

use std::{cell::Cell, rc::Rc};

use skene_reactive::Signal;

use super::{Behavior, ControlHandle, Controlled, PointerPhase};
use crate::{
  AccessProp,
  Axis,
  DragAxes,
  EventKind,
  Key,
  KeyEvent,
  Length,
  NamedKey,
  NativeElement,
  NodeId,
  Part,
  PointerButton,
  PointerEvent,
  Prevent,
  Realization,
  Role,
  Scalar,
  Scene,
  realize::Source,
};

/// A slider over a closed range.
///
/// The arrows move by a step, the page keys by a tenth of the range or a step where
/// the step is larger, `Home` and `End` reach the ends, and a press or drag along the
/// control's axis jumps to the value whose thumb centers under the pointer, measured
/// along the control's [`Part::Track`] where it has one and its box where it has none,
/// the thumb travelling that extent less [its own](Self::set_thumb_extent); a vertical
/// range runs bottom to top.
///
/// Controlled: the value it reports is what [`derive_value`](Self::derive_value)
/// reads, and a move only calls [`handle_change`](Self::handle_change) with the value
/// it asks for; [`bind_value`](Self::bind_value) wires both to one signal. A
/// non-positive step leaves the range continuous, and an arrow then moves by a
/// hundredth of the span.
#[must_use]
pub struct Slider {
  range: Source<SliderRange>,
  value: Controlled<f32>,
  extent: Rc<Cell<f32>>,
  track: Rc<Cell<Option<NodeId>>>,
  thumb: f32,
  pressed: Cell<bool>,
  axis: Axis,
}

impl std::fmt::Debug for Slider {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Slider").finish_non_exhaustive()
  }
}

impl Slider {
  /// A slider over `0..=100` stepping by `1`, holding its minimum.
  pub fn new() -> Self {
    Self {
      range: Source::Fixed(SliderRange::new(0., 100., 1.)),
      value: Controlled::new(),
      extent: Rc::new(Cell::new(0.)),
      track: Rc::new(Cell::new(None)),
      thumb: 0.,
      pressed: Cell::new(false),
      axis: Axis::Horizontal,
    }
  }

  /// Sets the axis the range runs along.
  #[inline(always)]
  pub const fn set_axis(mut self, axis: Axis) -> Self {
    self.axis = axis;

    self
  }

  /// Sets the extent of the thumb along the axis, a plain number in the medium's unit,
  /// landing in the unit as a length does (a terminal's `1.5` is two cells): the
  /// thumb's center travels from half of it past the start to half of it short of the
  /// end. Zero, the default, maps the pointer straight along the whole extent.
  #[inline(always)]
  pub fn set_thumb_extent(mut self, extent: impl Into<Length>) -> Self {
    self.thumb = extent.into().0;

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
    self.value.set(value);

    self
  }

  /// Derives the value from the signals `value` reads.
  pub fn derive_value(mut self, value: impl Fn() -> f32 + 'static) -> Self {
    self.value.derive(value);

    self
  }

  /// Runs `action` with the value a move asks for.
  pub fn handle_change(mut self, action: impl Fn(f32) + 'static) -> Self {
    self.value.handle(action);

    self
  }

  /// Binds the value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_value(mut self, signal: Signal<f32>) -> Self {
    self.value.bind(signal);

    self
  }

  /// The range as of now.
  #[inline(always)]
  fn range(&self) -> SliderRange {
    self.range.get()
  }

  /// The value currently reported.
  #[inline(always)]
  fn read(&self) -> f32 {
    self.value.read().unwrap_or_else(|| self.range().min())
  }

  /// Asks for `next` when it differs from the held value.
  fn write(&self, next: f32) {
    if (next - self.read()).abs() <= f32::EPSILON {
      return;
    }

    self.value.ask(next);
  }

  /// Asks for the value whose thumb centers under the pointer along the measured
  /// extent, the pointer measured from the track's start where the control has one.
  fn slide<P: Realization>(&self, event: &PointerEvent) {
    let event = self
      .track
      .get()
      .map_or(*event, |track| P::localize_pointer(track, event));
    let extent = self.extent.get();
    let thumb = P::Scalar::from_f32(self.thumb).to_f32().clamp(0., extent);
    let travel = extent - thumb;

    if travel > 0. {
      let along = match self.axis {
        Axis::Horizontal => event.local_x,
        Axis::Vertical => extent - event.local_y,
      };

      self.write(self.range().value_at((along - thumb / 2.) / travel));
    }
  }

  /// Where `key` takes the value; `None` when the key means nothing to a slider.
  fn key_value(&self, key: &Key) -> Option<f32> {
    let range = self.range();
    let current = self.read();
    let Key::Named(named) = key
    else {
      return None;
    };

    Some(match named {
      NamedKey::ArrowRight | NamedKey::ArrowUp => range.nudge(current, 1.),
      NamedKey::ArrowLeft | NamedKey::ArrowDown => range.nudge(current, -1.),
      NamedKey::PageUp => range.page(current, 1.),
      NamedKey::PageDown => range.page(current, -1.),
      NamedKey::Home => range.min(),
      NamedKey::End => range.max(),
      _ => return None,
    })
  }
}

impl Default for Slider {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Behavior<P> for Slider {
  #[inline(always)]
  fn native(&self) -> Option<NativeElement> {
    Some(NativeElement::Slider)
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::Slider)
  }

  #[inline(always)]
  fn drag_axes(&self) -> Option<DragAxes> {
    Some(match self.axis {
      Axis::Horizontal => DragAxes::Horizontal,
      Axis::Vertical => DragAxes::Vertical,
    })
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[
      EventKind::KeyDown,
      EventKind::PointerDown,
      EventKind::PointerMove,
      EventKind::PointerUp,
      EventKind::PointerCancel,
    ]
  }

  fn mounted(&self, control: &ControlHandle<P>) {
    let node = control.node();
    let track = Scene::<P>::with(|scene| scene.find_part(node, Part::Track));
    let extent = Rc::clone(&self.extent);
    let axis = self.axis;

    self.track.set(track);
    // The track where the control has one, the box where it has none.
    let _observed = P::observe_size(
      track.unwrap_or(node),
      Rc::new(move |size| {
        extent.set(match axis {
          Axis::Horizontal => size.width.to_f32(),
          Axis::Vertical => size.height.to_f32(),
        });
      }),
    );
  }

  fn on_key(&self, event: &KeyEvent, _control: &ControlHandle<P>) -> Prevent {
    let Some(next) = self.key_value(&event.key)
    else {
      return Prevent::None;
    };

    self.write(next);

    Prevent::All
  }

  fn on_pointer(
    &self,
    phase: PointerPhase,
    event: &PointerEvent,
    _control: &ControlHandle<P>,
  ) -> Prevent {
    let primary = event
      .button
      .is_none_or(|button| button == PointerButton::Primary);

    match phase {
      PointerPhase::Down => {
        if !primary {
          return Prevent::None;
        }

        self.pressed.set(true);
        self.slide::<P>(event);

        Prevent::All
      }
      PointerPhase::Move => {
        if !self.pressed.get() {
          return Prevent::None;
        }

        self.slide::<P>(event);

        Prevent::Bubbling
      }
      PointerPhase::Up => {
        if !primary || !self.pressed.replace(false) {
          return Prevent::None;
        }

        Prevent::Bubbling
      }
      PointerPhase::Cancel => {
        if !self.pressed.replace(false) {
          return Prevent::None;
        }

        Prevent::Bubbling
      }
    }
  }

  fn access(&self) -> Vec<AccessProp> {
    let range = self.range();

    vec![
      AccessProp::Value(self.read()),
      AccessProp::ValueMin(range.min()),
      AccessProp::ValueMax(range.max()),
      AccessProp::ValueStep(range.step()),
      AccessProp::Orientation(self.axis),
    ]
  }
}

/// A slider's value model: a closed range with an optional snap step.
///
/// A non-positive step snaps to nothing, leaving the range continuous.
#[must_use]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SliderRange {
  min: f32,
  max: f32,
  step: f32,
}

impl SliderRange {
  /// Creates a range over `min..=max`, snapping to multiples of `step` from `min`.
  ///
  /// The bounds are normalized as a browser normalizes a range input's: a `min` or
  /// `max` that is not a number stands at `0` or `100`, a `max` below `min` collapses
  /// onto `min`, and a `step` that is not a number leaves the range continuous.
  #[inline(always)]
  pub const fn new(min: f32, max: f32, step: f32) -> Self {
    let min = if min.is_nan() { 0. } else { min };
    let max = if max.is_nan() { 100. } else { max };
    let max = if max < min { min } else { max };
    let step = if step.is_nan() { 0. } else { step };

    Self { min, max, step }
  }

  /// The lowest value.
  #[inline(always)]
  #[must_use]
  pub const fn min(&self) -> f32 {
    self.min
  }

  /// The highest value.
  #[inline(always)]
  #[must_use]
  pub const fn max(&self) -> f32 {
    self.max
  }

  /// The step values snap to; non-positive when the range is continuous.
  #[inline(always)]
  #[must_use]
  pub const fn step(&self) -> f32 {
    self.step
  }

  /// The value `fraction` of the way along the track, snapped and clamped.
  ///
  /// `fraction` is clamped to `0..=1` first.
  #[must_use]
  pub fn value_at(&self, fraction: f32) -> f32 {
    let value = self.min + fraction.clamp(0., 1.) * (self.max - self.min);

    self.snap(value)
  }

  /// Where `value` sits along the track, `0..=1`—what a filled run measures.
  ///
  /// An empty range reads as `0`.
  #[must_use]
  pub fn fraction_of(&self, value: f32) -> f32 {
    let span = self.max - self.min;

    if span > 0. {
      ((value - self.min) / span).clamp(0., 1.)
    }
    else {
      0.
    }
  }

  /// `value` moved by `steps` steps, snapped and clamped.
  ///
  /// A continuous range moves by one hundredth of its span instead.
  #[must_use]
  pub fn nudge(&self, value: f32, steps: f32) -> f32 {
    let step = if self.step > 0. {
      self.step
    }
    else {
      (self.max - self.min) / 100.
    };

    self.snap(value + step * steps)
  }

  /// `value` moved by `pages` pages, snapped and clamped: a page is a tenth of the
  /// span, or a step where the step is larger.
  #[must_use]
  pub fn page(&self, value: f32, pages: f32) -> f32 {
    let page = ((self.max - self.min) / 10.).max(self.step);

    self.snap(value + page * pages)
  }

  /// `value` snapped to the nearest step and clamped into the range.
  #[must_use]
  fn snap(&self, value: f32) -> f32 {
    if self.step > 0. {
      (self.min + ((value - self.min) / self.step).round() * self.step).clamp(self.min, self.max)
    }
    else {
      value.clamp(self.min, self.max)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::SliderRange;

  /// Crossed or unparsed bounds are normalized, so nothing panics clamping into them.
  #[test]
  fn a_range_normalizes_its_bounds() {
    assert_eq!(
      SliderRange::new(10., 5., 1.),
      SliderRange::new(10., 10., 1.)
    );
    assert_eq!(
      SliderRange::new(f32::NAN, f32::NAN, f32::NAN),
      SliderRange::new(0., 100., 0.)
    );
    assert!((SliderRange::new(10., 5., 1.).value_at(0.5) - 10.).abs() < f32::EPSILON);
    assert!((SliderRange::new(10., 5., 0.).nudge(7., 1.) - 10.).abs() < f32::EPSILON);
  }

  /// A page is a tenth of the span, or a step where the step is larger, and lands on
  /// a step.
  #[test]
  fn a_page_is_a_tenth_of_the_span_or_a_step() {
    assert_eq!(SliderRange::new(0., 100., 5.).page(50., 1.), 60.);
    assert_eq!(SliderRange::new(0., 100., 3.).page(0., 1.), 9.);
    assert_eq!(SliderRange::new(0., 10., 3.).page(3., -1.), 0.);
    assert_eq!(SliderRange::new(0., 100., 40.).page(0., 1.), 40.);
    assert_eq!(SliderRange::new(0., 10., 0.).page(5., -1.), 4.);
  }

  /// A stepped range snaps a fraction to the nearest step and clamps at both ends.
  #[test]
  fn value_at_snaps_and_clamps() {
    let range = SliderRange::new(0., 100., 25.);

    assert_eq!(range.value_at(0.), 0.);
    assert_eq!(range.value_at(0.3), 25.);
    assert_eq!(range.value_at(1.), 100.);
    assert_eq!(range.value_at(2.), 100., "past the end lands on the end");
    assert_eq!(range.value_at(-1.), 0.);
  }

  /// A continuous range keeps the fraction exactly.
  #[test]
  fn a_continuous_range_does_not_snap() {
    let range = SliderRange::new(0., 10., 0.);

    assert_eq!(range.value_at(0.25), 2.5);
  }

  /// The fraction is the inverse of the value, and a degenerate range reads as empty.
  #[test]
  fn fraction_of_inverts_the_value() {
    let range = SliderRange::new(20., 60., 0.);

    assert_eq!(range.fraction_of(20.), 0.);
    assert_eq!(range.fraction_of(40.), 0.5);
    assert_eq!(range.fraction_of(60.), 1.);
    assert_eq!(
      range.fraction_of(80.),
      1.,
      "outside the range still reads full"
    );
    assert_eq!(
      SliderRange::new(5., 5., 0.).fraction_of(5.),
      0.,
      "an empty range paints no fill",
    );
  }

  /// A nudge moves by whole steps and stops at the ends.
  #[test]
  fn nudge_moves_by_steps_and_clamps() {
    let range = SliderRange::new(0., 10., 2.);

    assert_eq!(range.nudge(4., 1.), 6.);
    assert_eq!(range.nudge(4., -1.), 2.);
    assert_eq!(range.nudge(10., 1.), 10.);
    assert_eq!(range.nudge(0., -1.), 0.);
  }

  /// A range with no step still moves under an arrow key.
  #[test]
  fn a_continuous_range_still_nudges() {
    let range = SliderRange::new(0., 200., 0.);

    assert_eq!(range.nudge(50., 1.), 52.);
  }
}
