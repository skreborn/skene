//! A choice of a number within a range, drawn by the view it holds.

use std::{cell::Cell, rc::Rc};

use skene_core::{
  AccessProp,
  AnyView,
  FocusOrder,
  IntoView,
  KeyEvent,
  NodeId,
  PointerEvent,
  Prevent,
  Role,
  SliderRange,
  Source,
  View,
};
use skene_reactive::{Get, Set, Signal};

use crate::{
  Container,
  HasEvents,
  control::{Behavior, EventCore},
};

/// How many steps a page key moves, matching what a browser's own range input does.
const PAGE_STEPS: f32 = 10.;

/// A slider drawn by the view it holds: the control supplies the behavior, the child
/// supplies every pixel.
///
/// Keyboard and pointer handling, the tab order, the slider role, and the value it
/// reports come from the control; nothing is drawn for you, so the child renders the
/// track, the fill, and the thumb—derive them from the same signal the slider is bound
/// to. Arrows move by a step, `Home` and `End` reach the ends, the page keys move by
/// ten steps, and a press or drag along the control jumps to that position, matching
/// the browser's own. Every one of those is overridable, since a handler installed with
/// [`handle_key_down`](Self::handle_key_down) or the pointer trio runs ahead of it and
/// may take the event.
///
/// Because the child draws the fill, it follows the value with no script-bound
/// custom property and no vendor pseudo-element—the two things that make
/// [`WebSlider`](crate::WebSlider) hard to style.
#[must_use]
pub struct RawSlider<C = skene_core::NoChild> {
  child: C,
  min: f32,
  max: f32,
  step: f32,
  value: Option<Source<f32>>,
  change_handler: Option<Rc<dyn Fn(f32)>>,
  events: EventCore,
  focus_order: FocusOrder,
  label: Option<String>,
}

impl<C> HasEvents for RawSlider<C> {
  #[inline(always)]
  fn event_core(&mut self) -> &mut EventCore {
    &mut self.events
  }
}

impl RawSlider {
  /// Creates a slider over `min..=max`, stepping by 1.
  #[inline(always)]
  pub fn new(min: f32, max: f32) -> Self {
    Self {
      child: skene_core::NoChild,
      min,
      max,
      step: 1.,
      value: None,
      change_handler: None,
      events: EventCore::default(),
      focus_order: FocusOrder::Auto,
      label: None,
    }
  }
}

impl<C> RawSlider<C> {
  /// Sets the child that draws the control, which it requires before it can build.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView) -> RawSlider<AnyView> {
    RawSlider {
      child: child.into_view(),
      min: self.min,
      max: self.max,
      step: self.step,
      value: self.value,
      change_handler: self.change_handler,
      events: self.events,
      focus_order: self.focus_order,
      label: self.label,
    }
  }

  /// Sets the step a key press moves by, and that values snap to; a non-positive step
  /// leaves the range continuous.
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

  /// Sets a derived value, re-reported whenever it changes.
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

  /// Binds the value to `signal` in both directions: the slider reports the signal and
  /// each move writes it back.
  #[inline(always)]
  pub fn bind_value(self, signal: Signal<f32>) -> Self {
    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value))
  }

  /// Sets the control's place in sequential focus traversal.
  #[inline(always)]
  pub fn set_focus_order(mut self, focus_order: FocusOrder) -> Self {
    self.focus_order = focus_order;

    self
  }

  /// Sets the accessible name, for when the child draws no readable label.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }
}

impl View for RawSlider<AnyView> {
  fn build(self) -> NodeId {
    let range = SliderRange::new(self.min, self.max, self.step);
    let value = self.value;
    let label = self.label;
    let read = {
      let value = value.clone();

      move || value.as_ref().map_or(range.min(), Source::get)
    };
    let write: Rc<dyn Fn(f32)> = {
      let handler = self.change_handler.clone();

      Rc::new(move |next: f32| {
        if let Some(action) = &handler {
          action(next);
        }
      })
    };
    // The port's width, measured by the driver: a press maps to a fraction of it.
    let width = Signal::new(0.0f32);
    let pressed = Rc::new(Cell::new(false));
    let state = {
      let read = read.clone();
      let label = label.clone();

      move || {
        let mut state = vec![
          AccessProp::Value(read()),
          AccessProp::ValueMin(range.min()),
          AccessProp::ValueMax(range.max()),
        ];

        if let Some(label) = &label {
          state.push(AccessProp::Label(label.clone()));
        }

        state
      }
    };
    let slide = {
      let read = read.clone();
      let write = Rc::clone(&write);

      move |event: &PointerEvent| {
        let extent = width.get();

        if extent > 0. {
          let next = range.value_at(event.local_x / extent);

          if moved(next, read()) {
            write(next);
          }
        }
      }
    };
    let behavior = Behavior::new()
      .pointer_down({
        let pressed = Rc::clone(&pressed);
        let slide = slide.clone();

        move |event: &PointerEvent| {
          pressed.set(true);
          slide(event);

          Prevent::All
        }
      })
      .pointer_move({
        let pressed = Rc::clone(&pressed);

        move |event: &PointerEvent| {
          if !pressed.get() {
            return Prevent::None;
          }

          slide(event);

          Prevent::Bubbling
        }
      })
      .pointer_up({
        let pressed = Rc::clone(&pressed);

        move |_: &PointerEvent| {
          pressed.set(false);

          Prevent::Bubbling
        }
      })
      .key_down({
        let read = read.clone();
        let write = Rc::clone(&write);

        move |event: &KeyEvent| {
          let current = read();
          let Some(next) = key_value(range, current, &event.key)
          else {
            return Prevent::None;
          };

          if moved(next, current) {
            write(next);
          }

          Prevent::All
        }
      });
    let container = Container::new()
      .set_role(Role::Slider)
      .set_focus_order(self.focus_order)
      .derive_accessibility(state);
    let node = self
      .events
      .attach(container, behavior)
      .set_child(self.child)
      .build();

    crate::driver::observe_size(node, Rc::new(move |measured, _| width.set(measured)));

    node
  }
}

/// Where a key takes the value, or `None` when the key means nothing to a slider.
///
/// A slider rises on `Up` and `Right`, which is what the browser's own does.
fn key_value(range: SliderRange, current: f32, key: &str) -> Option<f32> {
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

/// Whether a new value differs from the held one, guarding the change handler against
/// a move that lands where the slider already sits.
#[inline(always)]
fn moved(next: f32, current: f32) -> bool {
  (next - current).abs() > f32::EPSILON
}
