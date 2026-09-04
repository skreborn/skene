//! A slider's behavior.

use std::{cell::Cell, rc::Rc};

use skene_core::{
  AccessProp,
  KeyEvent,
  PointerEvent,
  Prevent,
  Role,
  SliderRange,
  next::{ControlKind, Driver, EventKind, Scalar},
};
use skene_reactive::{Get, Set, Signal};

use super::{Behavior, ControlHandle, PointerPhase};

/// How many steps a page key moves.
const PAGE_STEPS: f32 = 10.;

/// A slider over a closed range: arrows move by a step, `Home` and `End` reach the
/// ends, the page keys move by ten steps, and a press or drag along the control's
/// width jumps to that fraction of the range.
///
/// Controlled: the value it reports is what [`derive_value`](Self::derive_value)
/// reads, and a move only calls [`handle_change`](Self::handle_change) with the value
/// it asks for; [`bind_value`](Self::bind_value) wires both to one signal. A
/// non-positive step leaves the range continuous, and an arrow then moves by a
/// hundredth of the span.
#[must_use]
pub struct Range {
  min: f32,
  max: f32,
  step: f32,
  value: Option<Rc<dyn Fn() -> f32>>,
  on_change: Option<Rc<dyn Fn(f32)>>,
  extent: Rc<Cell<f32>>,
  pressed: Cell<bool>,
}

impl Range {
  /// A slider over `0..=100` stepping by `1`, holding its minimum.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      min: 0.,
      max: 100.,
      step: 1.,
      value: None,
      on_change: None,
      extent: Rc::new(Cell::new(0.)),
      pressed: Cell::new(false),
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

  /// The range as the value model reads it.
  #[inline(always)]
  const fn range(&self) -> SliderRange {
    SliderRange::new(self.min, self.max, self.step)
  }

  /// The value currently reported.
  #[inline(always)]
  fn read(&self) -> f32 {
    self.value.as_ref().map_or(self.min, |value| value())
  }

  /// Asks for `next` when it differs from the held value.
  fn write(&self, next: f32) {
    if (next - self.read()).abs() <= f32::EPSILON {
      return;
    }

    if let Some(action) = &self.on_change {
      action(next);
    }
  }

  /// Asks for the value under the pointer, along the measured width.
  fn slide(&self, event: &PointerEvent) {
    let extent = self.extent.get();

    if extent > 0. {
      self.write(self.range().value_at(event.local_x / extent));
    }
  }

  /// Where `key` takes the value; `None` when the key means nothing to a slider.
  fn key_value(&self, key: &str) -> Option<f32> {
    let range = self.range();
    let current = self.read();

    Some(match key {
      "ArrowRight" | "ArrowUp" => range.nudge(current, 1.),
      "ArrowLeft" | "ArrowDown" => range.nudge(current, -1.),
      "PageUp" => range.nudge(current, PAGE_STEPS),
      "PageDown" => range.nudge(current, -PAGE_STEPS),
      "Home" => range.min(),
      "End" => range.max(),
      _ => return None,
    })
  }
}

impl Default for Range {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> Behavior<D> for Range {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    ControlKind::Range
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(Role::Slider)
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    &[
      EventKind::KeyDown,
      EventKind::PointerDown,
      EventKind::PointerMove,
      EventKind::PointerUp,
    ]
  }

  fn attach(&self, control: &ControlHandle<D>) {
    let extent = Rc::clone(&self.extent);

    control.with_sink(|sink| {
      D::default().observe_size(sink, Rc::new(move |size| extent.set(size.width.to_f32())));
    });
  }

  fn on_key(&self, event: &KeyEvent, _control: &ControlHandle<D>) -> Prevent {
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
    _control: &ControlHandle<D>,
  ) -> Prevent {
    match phase {
      PointerPhase::Down => {
        self.pressed.set(true);
        self.slide(event);

        Prevent::All
      }
      PointerPhase::Move => {
        if !self.pressed.get() {
          return Prevent::None;
        }

        self.slide(event);

        Prevent::Bubbling
      }
      PointerPhase::Up => {
        if !self.pressed.replace(false) {
          return Prevent::None;
        }

        Prevent::Bubbling
      }
    }
  }

  fn access(&self) -> Vec<AccessProp> {
    vec![
      AccessProp::Value(self.read()),
      AccessProp::ValueMin(self.min),
      AccessProp::ValueMax(self.max),
    ]
  }
}
