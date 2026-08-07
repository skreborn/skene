//! A dropdown choice among fixed options.

// View fields are read only by a platform's realization module; a target with no platform
// compiles the struct but no realizer, leaving them unread.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::rc::Rc;

use skene_core::{SelectOption, Source};

use crate::control::{ControlBase, control_surface};

/// A select: a dropdown choice among fixed options.
///
/// The chosen value flows both ways: [`derive_value`](Self::derive_value) (or
/// [`set_value`](Self::set_value)) drives the selection, and
/// [`handle_change`](Self::handle_change) receives each choice as it is made—
/// [`bind_value`](Self::bind_value) wires both to one signal. Unstyled by default, with
/// the platform's full keyboard behavior.
#[must_use]
pub struct Select {
  options: Vec<SelectOption>,
  value: Option<Source<String>>,
  #[expect(clippy::type_complexity)]
  change_handler: Option<Rc<dyn Fn(&str)>>,
  control: ControlBase,
}

impl Select {
  /// Creates a select with no options.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      options: Vec::new(),
      value: None,
      change_handler: None,
      control: ControlBase::new(),
    }
  }

  /// Sets the options, in order, each a `(value, label)` pair.
  #[inline(always)]
  pub fn set_options<V: Into<String>, L: Into<String>>(
    mut self,
    options: impl IntoIterator<Item = (V, L)>,
  ) -> Self {
    self.options = options
      .into_iter()
      .map(|(value, label)| SelectOption {
        value: value.into(),
        label: label.into(),
      })
      .collect();

    self
  }

  /// Sets a static selected value.
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.value = Some(Source::Static(value.into()));

    self
  }

  /// Sets a derived selected value, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.value = Some(Source::Derived(Rc::new(value)));

    self
  }

  /// Sets the change handler, run with the chosen value on every choice.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.change_handler = Some(Rc::new(action));

    self
  }
}

control_surface!(Select);

impl Default for Select {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(target_arch = "wasm32")]
impl Select {
  /// Binds the selection to `signal` in both directions: the select shows the signal
  /// and each choice writes it back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::Signal<String>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value.to_owned()))
  }
}

#[cfg(target_arch = "wasm32")]
impl skene_core::View for Select {
  fn build(self) -> skene_core::NodeId {
    use skene_core::SelectProps;

    use crate::driver;

    let (control, handlers) = self.control.into_parts();
    let node = driver::create_select(&SelectProps {
      options: self.options,
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
