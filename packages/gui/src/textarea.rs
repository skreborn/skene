//! A multi-line text area.

// View fields are read only by a platform's realization module; a target with no platform
// compiles the struct but no realizer, leaving them unread.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::rc::Rc;

use skene_core::Source;

use crate::control::{ControlBase, control_surface};

/// A multi-line text area.
///
/// The value flows both ways: [`derive_value`](Self::derive_value) (or
/// [`set_value`](Self::set_value)) drives the shown text, and
/// [`handle_input`](Self::handle_input) receives each edit—
/// [`bind_value`](Self::bind_value) wires both to one signal. Unstyled by default; the
/// visible height is set in text rows with [`set_rows`](Self::set_rows).
#[must_use]
pub struct Textarea {
  value: Option<Source<String>>,
  placeholder: Option<String>,
  rows: Option<u16>,
  #[expect(clippy::type_complexity)]
  input_handler: Option<Rc<dyn Fn(&str)>>,
  #[expect(clippy::type_complexity)]
  change_handler: Option<Rc<dyn Fn(&str)>>,
  control: ControlBase,
}

impl Textarea {
  /// Creates an empty text area.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      value: None,
      placeholder: None,
      rows: None,
      input_handler: None,
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

  /// Sets the visible height, in text rows.
  #[inline(always)]
  pub fn set_rows(mut self, rows: u16) -> Self {
    self.rows = Some(rows);

    self
  }

  /// Sets the edit handler, run with the new value on every change.
  #[inline(always)]
  pub fn handle_input(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.input_handler = Some(Rc::new(action));

    self
  }

  /// Sets the change handler, run with the committed value when an edit is committed
  /// (focus leaves the text area).
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.change_handler = Some(Rc::new(action));

    self
  }
}

control_surface!(Textarea);

impl Default for Textarea {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(target_arch = "wasm32")]
impl Textarea {
  /// Binds the value to `signal` in both directions: the text area shows the signal and
  /// each edit writes it back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::RwSignal<String>) -> Self {
    use skene_reactive::{SignalGet, SignalUpdate};

    self
      .derive_value(move || signal.get())
      .handle_input(move |value| signal.set(value.to_owned()))
  }
}

#[cfg(target_arch = "wasm32")]
impl skene_core::View for Textarea {
  fn build(self) -> skene_core::NodeId {
    use skene_core::TextareaProps;

    use crate::driver;

    let (control, handlers) = self.control.into_parts();
    let node = driver::create_textarea(&TextareaProps {
      placeholder: self.placeholder,
      rows: self.rows,
      control,
    });

    if let Some(value) = self.value {
      value.bind(move |value| driver::set_input_value(node, &value));
    }

    if let Some(action) = self.input_handler {
      driver::handle_input(node, action);
    }

    if let Some(action) = self.change_handler {
      driver::handle_change(node, action);
    }

    handlers.register(node);

    node
  }
}
