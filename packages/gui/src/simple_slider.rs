//! A ready-made slider in the Simple design language.

use skene_core::View;

use crate::{RawSlider, simple::simple_surface};

/// A ready-made slider: the platform's ink-drawn track and thumb.
///
/// The value flows both ways, exactly as on [`RawSlider`]; for custom styling, compose
/// the raw control directly.
#[must_use]
pub struct SimpleSlider {
  raw: RawSlider,
}

impl SimpleSlider {
  /// Creates a slider selecting within `min..=max`.
  #[inline(always)]
  pub fn new(min: f32, max: f32) -> Self {
    Self {
      raw: RawSlider::new(min, max),
    }
  }

  /// Sets the step between selectable values.
  #[inline(always)]
  pub fn set_step(mut self, step: f32) -> Self {
    self.raw = self.raw.set_step(step);

    self
  }

  /// Sets a static value.
  #[inline(always)]
  pub fn set_value(mut self, value: f32) -> Self {
    self.raw = self.raw.set_value(value);

    self
  }

  /// Sets a derived value, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> f32 + 'static) -> Self {
    self.raw = self.raw.derive_value(value);

    self
  }

  /// Sets the change handler, run with the new value on every move, in real time.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(f32) + 'static) -> Self {
    self.raw = self.raw.handle_change(action);

    self
  }

  /// Binds the value to `signal` in both directions: the slider shows the signal and
  /// each move writes it back.
  #[inline(always)]
  pub fn bind_value(mut self, signal: skene_reactive::Signal<f32>) -> Self {
    self.raw = self.raw.bind_value(signal);

    self
  }
}

simple_surface!(SimpleSlider);

impl View for SimpleSlider {
  fn build(self) -> skene_core::NodeId {
    self.raw.build()
  }
}
