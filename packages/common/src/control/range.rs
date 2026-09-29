//! A slider's behavior.

use std::{cell::Cell, rc::Rc};

use skene_core::{
  AccessProp,
  Axis,
  ControlKind,
  DragAxes,
  EventKind,
  Key,
  KeyEvent,
  NamedKey,
  NodeId,
  Part,
  Platform,
  PointerButton,
  PointerEvent,
  Prevent,
  Role,
  Scalar,
  Scene,
  SliderRange,
};
use skene_reactive::Signal;

use super::{Behavior, ControlHandle, Controlled, PointerPhase};
use crate::realize::with_sink;

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
pub struct Range {
  min: f32,
  max: f32,
  step: f32,
  value: Controlled<f32>,
  extent: Rc<Cell<f32>>,
  track: Rc<Cell<Option<NodeId>>>,
  thumb: f32,
  pressed: Cell<bool>,
  axis: Axis,
}

impl std::fmt::Debug for Range {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Range").finish_non_exhaustive()
  }
}

impl Range {
  /// A slider over `0..=100` stepping by `1`, holding its minimum.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      min: 0.,
      max: 100.,
      step: 1.,
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

  /// Sets the extent of the thumb along the axis, in the medium's unit: the thumb's
  /// center travels from half of it past the start to half of it short of the end.
  /// Zero, the default, maps the pointer straight along the whole extent.
  #[inline(always)]
  pub const fn set_thumb_extent(mut self, extent: f32) -> Self {
    self.thumb = extent;

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
    self.value.set(value);

    self
  }

  /// Derives the value from the signals `value` reads.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> f32 + 'static) -> Self {
    self.value.derive(value);

    self
  }

  /// Runs `action` with the value a move asks for.
  #[inline(always)]
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

  /// The range as the value model reads it.
  #[inline(always)]
  const fn range(&self) -> SliderRange {
    SliderRange::new(self.min, self.max, self.step)
  }

  /// The value currently reported.
  #[inline(always)]
  fn read(&self) -> f32 {
    self.value.read().unwrap_or(self.min)
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
  fn slide<P: Platform>(&self, event: &PointerEvent) {
    let event = self
      .track
      .get()
      .and_then(|track| with_sink::<P, _>(track, |sink| P::localize_pointer(sink, event)))
      .unwrap_or(*event);
    let extent = self.extent.get();
    let thumb = self.thumb.clamp(0., extent);
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

impl Default for Range {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Behavior<P> for Range {
  #[inline(always)]
  fn kind(&self) -> ControlKind {
    ControlKind::Range
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
    with_sink::<P, _>(track.unwrap_or(node), |sink| {
      let _observed = P::observe_size(
        sink,
        Rc::new(move |size| {
          extent.set(match axis {
            Axis::Horizontal => size.width.to_f32(),
            Axis::Vertical => size.height.to_f32(),
          });
        }),
      );
    });
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
    vec![
      AccessProp::Value(self.read()),
      AccessProp::ValueMin(self.min),
      AccessProp::ValueMax(self.max),
      AccessProp::ValueStep(self.step),
      AccessProp::Orientation(self.axis),
    ]
  }
}
