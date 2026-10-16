//! A value on a range.

use std::rc::Rc;

use crate::{
  Signal,
  behavior::{Behavior, ControlHandle, Controlled, Response},
  layout::Axis,
  seam::{
    access::{AccessFacts, RangeValue, Role},
    content::Range,
    control::{AccessRequest, DragAxes, LabelPlacement, Part, Prevent},
    report::{KeyEvent, PointerEvent, PointerPhase},
  },
};

/// A slider's key table over its range: the arrows step the value (right and up increase
/// it), `PageUp` and `PageDown` move it by a tenth of the range (at least a step), `Home` and
/// `End` to its ends.
///
/// Its act is the value moved to, snapped to the step and clamped to the bounds, and only
/// where it differs from the value shown.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub struct SliderKeys {
  /// The value with its bounds and step.
  pub range: Range,
}

impl SliderKeys {
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

  /// The value one step up (`up`) or down, proposed.
  #[must_use]
  pub fn stepped(&self, up: bool) -> Option<f64> {
    let step = self.range.step.max(f64::EPSILON);

    self.moved(self.range.value + if up { step } else { -step })
  }

  /// A key pressed while the slider holds focus.
  pub fn key(&self, event: &KeyEvent) -> Response<f64> {
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
}

/// A slider's behavior: [`SliderKeys`]; a press on its track ([`Part::Track`]) jumps to where
/// it lands and a drag follows the pointer; a reader steps it or sets its value. Its label
/// sits beside it.
#[must_use]
pub struct Slider {
  value: Controlled<f64>,
  range: Rc<dyn Fn() -> (f64, f64, f64)>,
  dragging: Signal<bool>,
}

impl Default for Slider {
  fn default() -> Self {
    Self::new()
  }
}

impl Slider {
  /// A slider at 0 of 0 to 1, stepping by a hundredth.
  pub fn new() -> Self {
    Self {
      value: Controlled::new(),
      range: Rc::new(|| (0., 1., 0.01)),
      dragging: Signal::new(false),
    }
  }

  /// This slider, showing what `value` returns, kept current.
  pub fn derive_value(mut self, value: impl Fn() -> f64 + 'static) -> Self {
    self.value.derive(value);

    self
  }

  /// This slider, asking `change` for each value the reader moves it to.
  pub fn handle_change(mut self, change: impl Fn(f64) + 'static) -> Self {
    self.value.handle(change);

    self
  }

  /// This slider, its least value, its greatest and its step what `range` returns, kept
  /// current.
  pub fn derive_range(mut self, range: impl Fn() -> (f64, f64, f64) + 'static) -> Self {
    self.range = Rc::new(range);

    self
  }

  /// The value with its bounds and step, as they read now.
  #[must_use]
  pub fn range(&self) -> Range {
    let (minimum, maximum, step) = (self.range)();

    Range {
      value: self.value.read().unwrap_or(minimum),
      minimum,
      maximum,
      step,
    }
  }

  fn keys(&self) -> SliderKeys {
    SliderKeys::new(crate::untrack(|| self.range()))
  }

  fn jump(&self, event: &PointerEvent, control: &ControlHandle) {
    let Some(track) = control.frame(Part::Track) else {
      return;
    };
    let fraction = f64::from((event.window_x - track.x) / track.width.max(1.));

    if let Some(value) = self.keys().at(fraction) {
      self.value.ask(value);
    }
  }
}

impl Behavior for Slider {
  fn role(&self) -> Role {
    Role::Slider
  }

  fn drag_axes(&self) -> Option<DragAxes> {
    Some(DragAxes::Horizontal)
  }

  fn label_placement(&self) -> LabelPlacement {
    LabelPlacement::Beside
  }

  fn on_key(&self, event: &KeyEvent, _: &ControlHandle) -> Prevent {
    let response = self.keys().key(event);

    if let Some(value) = response.act {
      self.value.ask(value);
    }

    response.verdict()
  }

  fn on_pointer(&self, event: &PointerEvent, control: &ControlHandle) -> Prevent {
    match event.phase {
      PointerPhase::Down if event.button == 0 => {
        self.dragging.set(true);
        self.jump(event, control);

        Prevent::All
      }
      PointerPhase::Move if self.dragging.get_untracked() => {
        self.jump(event, control);

        Prevent::All
      }
      PointerPhase::Up | PointerPhase::Cancel if self.dragging.get_untracked() => {
        self.dragging.set(false);

        Prevent::All
      }
      _ => Prevent::None,
    }
  }

  fn on_access(&self, request: AccessRequest, _: &ControlHandle) -> bool {
    let keys = self.keys();
    let value = match request {
      AccessRequest::Increment => keys.stepped(true),
      AccessRequest::Decrement => keys.stepped(false),
      AccessRequest::SetValue(value) => keys.moved(value),
      _ => return false,
    };

    if let Some(value) = value {
      self.value.ask(value);
    }

    true
  }

  fn access(&self) -> AccessFacts {
    let range = self.range();

    AccessFacts {
      value: Some(RangeValue {
        now: range.value,
        minimum: range.minimum,
        maximum: range.maximum,
        step: Some(range.step),
        text: None,
      }),
      orientation: Some(Axis::Horizontal),
      actions: vec![AccessRequest::Increment, AccessRequest::Decrement],
      ..AccessFacts::default()
    }
  }
}
