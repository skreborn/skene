//! A single-line text input.

use std::rc::Rc;

use skene_core::Source;

use crate::control::{ControlBase, control_surface};

/// A single-line text input.
///
/// The value flows both ways: [`derive_value`](Self::derive_value) (or
/// [`set_value`](Self::set_value)) drives the shown text, and
/// [`handle_change`](Self::handle_change) receives each edit in real time—
/// [`bind_value`](Self::bind_value) wires both to one signal. Styling is the box style,
/// set wholesale with `set_style` and conditionally with `set_when`.
///
/// ```ignore
/// let name = Signal::new(String::new());
///
/// Input::new()
///   .set_placeholder("Name")
///   .bind_value(name)
/// ```
#[must_use]
pub struct Input {
  value: Option<Source<String>>,
  placeholder: Option<String>,
  #[expect(clippy::type_complexity)]
  change_handler: Option<Rc<dyn Fn(&str)>>,
  control: ControlBase,
}

impl Input {
  /// Creates an empty input.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      value: None,
      placeholder: None,
      change_handler: None,
      control: ControlBase::new(),
    }
  }

  /// Sets a static value.
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.value = Some(Source::Static(value.into()));

    self
  }

  /// Sets a derived value, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.value = Some(Source::Derived(Rc::new(value)));

    self
  }

  /// Sets the hint shown while the value is empty.
  #[inline(always)]
  pub fn set_placeholder(mut self, placeholder: impl Into<String>) -> Self {
    self.placeholder = Some(placeholder.into());

    self
  }

  /// Sets the change handler, run with the new value on every edit, in real time.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.change_handler = Some(Rc::new(action));

    self
  }
}

control_surface!(Input);

impl Default for Input {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Input {
  /// Binds the value to `signal` in both directions: the input shows the signal and each
  /// edit writes it back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::Signal<String>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value.to_owned()))
  }
}

impl skene_core::View for Input {
  fn build(self) -> skene_core::NodeId {
    use skene_core::InputProps;

    use crate::driver;

    let (control, handlers) = self.control.into_parts();
    let node = driver::create_input(&InputProps {
      placeholder: self.placeholder,
      control,
    });

    if let Some(value) = self.value {
      value.bind(move |value| driver::set_input_value(node, &value));
    }

    if let Some(action) = self.change_handler {
      driver::handle_change(node, action);
    }

    handlers.register(node);

    node
  }
}
