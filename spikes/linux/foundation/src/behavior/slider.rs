//! A value on a range.

use crate::{
  behavior::{Behavior, Response},
  seam::{
    access::{AccessFacts, Action, RangeValue, Role},
    content::Range,
    report::KeyEvent,
  },
};

/// A slider's behavior over its range: the arrows step the value (right and up increase it),
/// `PageUp` and `PageDown` move it by a tenth of the range (at least a step), `Home` and `End`
/// to its ends; a press at a point of the track jumps there and a drag follows the pointer.
///
/// It proposes the value moved to, snapped to the step and clamped to the bounds, and only
/// where it differs from the value shown.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub struct Slider {
  /// The value with its bounds and step.
  pub range: Range,
}

impl Slider {
  /// A slider over `range`.
  #[inline(always)]
  pub const fn new(range: Range) -> Self {
    Self { range }
  }

  /// `value` snapped to the step from the least value and clamped to the bounds.
  #[must_use]
  pub fn snap(&self, value: f64) -> f64 {
    let range = self.range;
    let stepped = if range.step > 0. {
      range.minimum + ((value - range.minimum) / range.step).round() * range.step
    } else {
      value
    };

    stepped.clamp(range.minimum, range.maximum.max(range.minimum))
  }

  /// The value moved to `value`, where it differs from the one shown.
  #[must_use]
  pub fn moved(&self, value: f64) -> Option<f64> {
    let value = self.snap(value);

    ((value - self.range.value).abs() > f64::EPSILON).then_some(value)
  }

  /// How far the page keys move the value: a tenth of the range, at least a step.
  #[must_use]
  pub fn page(&self) -> f64 {
    ((self.range.maximum - self.range.minimum) / 10.).max(self.range.step)
  }

  /// The value at `fraction` of the track (0 its start, 1 its end), proposed.
  #[must_use]
  pub fn at(&self, fraction: f64) -> Option<f64> {
    let span = self.range.maximum - self.range.minimum;

    self.moved(self.range.minimum + fraction.clamp(0., 1.) * span)
  }

  /// Where the value shown sits along the track, from 0 to 1.
  #[must_use]
  pub fn fraction(&self) -> f64 {
    let span = (self.range.maximum - self.range.minimum).max(f64::EPSILON);

    ((self.range.value - self.range.minimum) / span).clamp(0., 1.)
  }
}

impl Behavior for Slider {
  type Act = f64;

  fn role(&self) -> Role {
    Role::Slider
  }

  fn key(&self, event: &KeyEvent) -> Response<f64> {
    let modifiers = event.modifiers;

    if !event.down || modifiers.control || modifiers.alt || modifiers.meta {
      return Response::IGNORED;
    }

    let range = self.range;
    let step = if range.step > 0. {
      range.step
    } else {
      (range.maximum - range.minimum) / 100.
    };
    let value = match event.key.as_str() {
      "ArrowRight" | "ArrowUp" => range.value + step,
      "ArrowLeft" | "ArrowDown" => range.value - step,
      "PageUp" => range.value + self.page(),
      "PageDown" => range.value - self.page(),
      "Home" => range.minimum,
      "End" => range.maximum,
      _ => return Response::IGNORED,
    };

    Response::maybe(self.moved(value))
  }

  fn action(&self, action: Action) -> Option<f64> {
    let step = self.range.step.max(f64::EPSILON);

    match action {
      Action::Increment => self.moved(self.range.value + step),
      Action::Decrement => self.moved(self.range.value - step),
      _ => None,
    }
  }

  fn actions(&self) -> Vec<Action> {
    vec![Action::Increment, Action::Decrement]
  }

  fn facts(&self) -> AccessFacts {
    AccessFacts {
      value: Some(RangeValue {
        now: self.range.value,
        minimum: self.range.minimum,
        maximum: self.range.maximum,
        text: None,
      }),
      ..AccessFacts::default()
    }
  }
}
