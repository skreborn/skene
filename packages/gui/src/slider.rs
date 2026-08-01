//! A draggable choice of a number within a range.

// View fields are read only by a platform's realization module; a target with no platform
// compiles the struct but no realizer, leaving them unread.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::rc::Rc;

use skene_core::Source;

use crate::control::{ControlBase, control_surface};

/// A slider: a draggable choice of a number within a range.
///
/// The value flows both ways: [`derive_value`](Self::derive_value) (or
/// [`set_value`](Self::set_value)) drives the position, and
/// [`handle_change`](Self::handle_change) receives each move in real time—
/// [`bind_value`](Self::bind_value) wires both to one signal. Unstyled by default, with
/// the platform's full keyboard behavior.
#[must_use]
pub struct Slider {
  min: f32,
  max: f32,
  step: Option<f32>,
  value: Option<Source<f32>>,
  change_handler: Option<Rc<dyn Fn(f32)>>,
  control: ControlBase,
}

impl Slider {
  /// Creates a slider selecting within `min..=max`.
  #[inline(always)]
  pub fn new(min: f32, max: f32) -> Self {
    Self {
      min,
      max,
      step: None,
      value: None,
      change_handler: None,
      control: ControlBase::new(),
    }
  }

  /// Sets the step between selectable values.
  #[inline(always)]
  pub fn set_step(mut self, step: f32) -> Self {
    self.step = Some(step);

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

  /// Sets the change handler, run with the new value on every move, in real time.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(f32) + 'static) -> Self {
    self.change_handler = Some(Rc::new(action));

    self
  }
}

control_surface!(Slider);

#[cfg(target_arch = "wasm32")]
impl Slider {
  /// Binds the value to `signal` in both directions: the slider shows the signal and
  /// each move writes it back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::RwSignal<f32>) -> Self {
    use skene_reactive::{SignalGet, SignalUpdate};

    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value))
  }
}

#[cfg(target_arch = "wasm32")]
impl skene_core::View for Slider {
  fn build(self) -> skene_core::NodeId {
    use skene_core::SliderProps;

    use crate::driver;

    let (control, handlers) = self.control.into_parts();
    let node = driver::create_slider(&SliderProps {
      min: self.min,
      max: self.max,
      step: self.step,
      control,
    });

    if let Some(value) = self.value {
      value.bind(move |value| driver::set_input_value(node, &value.to_string()));
    }

    if let Some(action) = self.change_handler {
      let action = Rc::clone(&action);

      driver::handle_change(
        node,
        Rc::new(move |value: &str| {
          if let Ok(value) = value.parse::<f32>() {
            action(value);
          }
        }),
      );
    }

    handlers.register(node);

    node
  }
}
