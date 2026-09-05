//! What a control is to the scene and to a driver: its kind, the state a box cannot
//! hold, and the parts it exposes.

use std::fmt::Debug;

use super::{BoxProps, Driver, State};
use crate::Toggle;

/// The state a control holds that a box cannot: what the control-scoped [`State`]
/// conditions of the control and everything inside it resolve against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct ControlState {
  /// Whether the control is checked.
  pub checked: Toggle,
  /// Whether the control is present but not operable.
  pub disabled: bool,
  /// Whether the control holds an invalid value.
  pub invalid: bool,
  /// Whether the control's popup is open.
  pub open: bool,
}

impl ControlState {
  /// Unchecked, enabled, valid and closed.
  pub const DEFAULT: Self = Self {
    checked: Toggle::Off,
    disabled: false,
    invalid: false,
    open: false,
  };

  /// Whether `state` holds; `false` for a state that is a node's own rather than the
  /// control's.
  #[inline(always)]
  #[must_use]
  pub const fn holds(self, state: State) -> bool {
    match state {
      State::Checked => matches!(self.checked, Toggle::On),
      State::Indeterminate => matches!(self.checked, Toggle::Mixed),
      State::Disabled => self.disabled,
      State::Invalid => self.invalid,
      State::Open => self.open,
      State::Hovered | State::Focused | State::FocusVisible | State::Pressed => false,
    }
  }
}

impl Default for ControlState {
  #[inline(always)]
  fn default() -> Self {
    Self::DEFAULT
  }
}

/// What a control is, guiding a driver that realizes some kinds natively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum ControlKind {
  /// A control that activates on press.
  Button,
  /// A control toggling between checked and unchecked.
  Checkbox,
  /// The group a set of mutually-exclusive choices belongs to.
  RadioGroup,
  /// One mutually-exclusive choice within a group.
  Radio,
  /// A control picking a number within a range.
  Range,
  /// A single-line text entry.
  TextField,
  /// A multi-line text entry.
  TextEditor,
  /// A list of choices to select from.
  Listbox,
  /// One choice within a listbox.
  ListboxOption,
  /// A control pairing a text entry with a list of choices.
  Combobox,
}

/// Everything a driver needs to realize a control: a box with a kind and, where
/// nothing visible supplies one, an accessible name.
#[must_use]
pub struct ControlProps<D: Driver> {
  /// What the control is.
  pub kind: ControlKind,
  /// The box the control is realized as.
  pub box_props: BoxProps<D>,
  /// The accessible name, when nothing visible supplies one.
  pub label: Option<String>,
  /// The hint a text entry shows while its value is empty.
  pub placeholder: Option<String>,
  /// The lines a text entry shows at once; `1` for every other kind.
  pub lines: u16,
}

impl<D: Driver> Clone for ControlProps<D> {
  fn clone(&self) -> Self {
    Self {
      kind: self.kind,
      box_props: self.box_props.clone(),
      label: self.label.clone(),
      placeholder: self.placeholder.clone(),
      lines: self.lines,
    }
  }
}

impl<D: Driver> Debug for ControlProps<D> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ControlProps")
      .field("kind", &self.kind)
      .field("label", &self.label)
      .field("placeholder", &self.placeholder)
      .field("lines", &self.lines)
      .finish_non_exhaustive()
  }
}

/// A styling target a control exposes: the node a part is recorded on is that part of
/// the nearest enclosing control.
///
/// A part's focus is its control's: `Focused`, `FocusVisible` and `Pressed` on a node
/// inside a control resolve against the control, while `Hovered` stays the node's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Part {
  /// The control's own node.
  Root,
  /// The extent a range's value moves along.
  Track,
  /// The portion of the track up to the value.
  Fill,
  /// The handle at the value.
  Thumb,
  /// The mark a toggle shows while checked.
  Mark,
  /// The face a toggle or button draws.
  Face,
  /// The list of choices a popup shows.
  List,
  /// One choice within the list.
  Item,
  /// The caption naming the control.
  Label,
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
  #[inline(always)]
  pub const fn new(min: f32, max: f32, step: f32) -> Self {
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
  /// `fraction` is clamped to `0..=1` first, so a pointer past either end lands on that
  /// end rather than outside the range.
  #[must_use]
  pub fn value_at(&self, fraction: f32) -> f32 {
    let value = self.min + fraction.clamp(0., 1.) * (self.max - self.min);

    self.snap(value)
  }

  /// Where `value` sits along the track, `0..=1`—what a filled run measures.
  ///
  /// An empty range reads as `0`, so a degenerate slider paints no fill rather than a
  /// full one.
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
  /// A continuous range moves by one hundredth of its span instead, so an arrow key
  /// still moves a slider that declares no step.
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

/// A ring of positions stepped through by wrapping moves: a radio group's members, a
/// picker's options.
#[must_use]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ring(usize);

impl Ring {
  /// Creates a ring of `count` positions.
  #[inline(always)]
  pub const fn new(count: usize) -> Self {
    Self(count)
  }

  /// The position `step` places from `from`, wrapping at both ends.
  ///
  /// `None` when the ring is empty, or when the arithmetic does not fit—there is no
  /// position to move to in either case.
  #[must_use]
  pub fn step_from(&self, from: usize, step: isize) -> Option<usize> {
    if self.0 == 0 {
      return None;
    }

    let count = isize::try_from(self.0).ok()?;
    let from = isize::try_from(from).ok()?;

    usize::try_from((from + step).rem_euclid(count)).ok()
  }
}

#[cfg(test)]
// Every expected value here is exactly representable—a step multiple or an end of the
// range—so an exact comparison is the assertion, not an approximation of it.
#[expect(clippy::float_cmp)]
mod tests {
  use super::{Ring, SliderRange};

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

  /// Stepping wraps at both ends, and an empty ring has nowhere to go.
  #[test]
  fn stepping_wraps_at_both_ends() {
    let ring = Ring::new(3);

    assert_eq!(ring.step_from(0, 1), Some(1));
    assert_eq!(ring.step_from(2, 1), Some(0), "forward past the end wraps");
    assert_eq!(ring.step_from(0, -1), Some(2), "back past the start wraps");
    assert_eq!(ring.step_from(0, 5), Some(2), "a long step still wraps");
    assert_eq!(Ring::new(0).step_from(0, 1), None);
  }
}
