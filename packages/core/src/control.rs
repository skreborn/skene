//! The medium-agnostic control state machines: the interaction a control *is*, apart
//! from how any facade draws it.
//!
//! A terminal has no native controls and draws its own; a GUI driver that draws its own
//! needs the same answers. These types hold that logic once, over a normalized fraction
//! rather than any medium's units, so each facade converts its own cells or pixels at
//! the edge and shares everything after.

use std::{
  cell::{Cell, RefCell},
  marker::PhantomData,
  rc::Rc,
};

use crate::{NodeId, TreeOps};

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

/// The members a grouping control tracks: a radio group's choices, a picker's options.
///
/// A member joins as it builds and leaves as it is disposed. `T` is whatever the facade
/// needs to act on one—on a GUI, the handle that focuses it.
///
/// Order comes from the tree, not from the order members joined: [`ordered`](Self::ordered)
/// walks the subtree under the [root](Self::root_at) and reports the members it meets. A
/// set that reconciles therefore stays in document order however it changes, and a
/// destroyed member is absent because the walk cannot reach it.
#[must_use]
pub struct Roster<Ops, T> {
  members: Rc<RefCell<Vec<(NodeId, String, T)>>>,
  root: Rc<Cell<Option<NodeId>>>,
  ops: PhantomData<Ops>,
}

impl<Ops: TreeOps, T> Roster<Ops, T> {
  /// Creates a roster with no members and no root.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets the node whose subtree holds the members, which orders them.
  ///
  /// Until one is set a roster reports its members in join order, since there is
  /// nothing to walk.
  #[inline(always)]
  pub fn root_at(&self, node: NodeId) {
    self.root.set(Some(node));
  }

  /// Adds the member `node` holding `value`, replacing any member already at that node.
  pub fn join(&self, node: NodeId, value: impl Into<String>, member: T) {
    let mut members = self.members.borrow_mut();

    members.retain(|(held, _, _)| *held != node);
    members.push((node, value.into(), member));
  }

  /// Removes the member at `node`.
  pub fn leave(&self, node: NodeId) {
    self
      .members
      .borrow_mut()
      .retain(|(held, _, _)| *held != node);
  }
}

impl<Ops: TreeOps, T: Clone> Roster<Ops, T> {
  /// The members in document order.
  ///
  /// One walk answers every question about the set, so a caller that needs a position,
  /// a count, and a neighbour takes this once rather than asking three times.
  #[must_use]
  pub fn ordered(&self) -> Vec<(String, T)> {
    let members = self.members.borrow();
    let Some(root) = self.root.get()
    else {
      return members
        .iter()
        .map(|(_, value, member)| (value.clone(), member.clone()))
        .collect();
    };
    let mut ordered = Vec::with_capacity(members.len());
    let mut pending = vec![root];

    while let Some(node) = pending.pop() {
      if let Some((_, value, member)) = members.iter().find(|(held, _, _)| *held == node) {
        ordered.push((value.clone(), member.clone()));
      }

      // Reversed, so the children come back off the stack in document order.
      pending.extend(Ops::default().children(node).into_iter().rev());
    }

    ordered
  }
}

impl<Ops, T> Clone for Roster<Ops, T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      members: Rc::clone(&self.members),
      root: Rc::clone(&self.root),
      ops: PhantomData,
    }
  }
}

impl<Ops, T> Default for Roster<Ops, T> {
  #[inline(always)]
  fn default() -> Self {
    Self {
      members: Rc::new(RefCell::new(Vec::new())),
      root: Rc::new(Cell::new(None)),
      ops: PhantomData,
    }
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
