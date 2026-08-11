//! A two-state toggle.

use std::rc::Rc;

use skene_core::Source;

use crate::control::{ControlBase, control_surface};

/// A checkbox: a two-state toggle.
///
/// The state flows both ways: [`derive_checked`](Self::derive_checked) (or
/// [`set_checked`](Self::set_checked)) drives the shown state, and
/// [`handle_toggle`](Self::handle_toggle) receives each change—
/// [`bind_checked`](Self::bind_checked) wires both to one signal. Unstyled by default,
/// with the platform's full keyboard behavior; styling is the box style, set with
/// `set_style` and conditionally with `set_when`.
#[must_use]
pub struct Checkbox {
  checked: Option<Source<bool>>,
  label: Option<String>,
  toggle_handler: Option<Rc<dyn Fn(bool)>>,
  control: ControlBase,
}

impl Checkbox {
  /// Creates an unchecked checkbox.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      checked: None,
      label: None,
      toggle_handler: None,
      control: ControlBase::new(),
    }
  }

  /// Sets the label beside the box; clicking it toggles too.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }

  /// Sets a static checked state.
  #[inline(always)]
  pub fn set_checked(mut self, checked: bool) -> Self {
    self.checked = Some(Source::Static(checked));

    self
  }

  /// Sets a derived checked state, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_checked(mut self, checked: impl Fn() -> bool + 'static) -> Self {
    self.checked = Some(Source::Derived(Rc::new(checked)));

    self
  }

  /// Sets the toggle handler, run with the new state on every change.
  #[inline(always)]
  pub fn handle_toggle(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.toggle_handler = Some(Rc::new(action));

    self
  }
}

control_surface!(Checkbox);

impl Default for Checkbox {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Checkbox {
  /// Binds the state to `signal` in both directions: the checkbox shows the signal and
  /// each toggle writes it back.
  #[inline(always)]
  pub fn bind_checked(self, signal: skene_reactive::Signal<bool>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_checked(move || signal.get())
      .handle_toggle(move |checked| signal.set(checked))
  }
}

impl skene_core::View for Checkbox {
  fn build(self) -> skene_core::NodeId {
    use skene_core::CheckboxProps;

    use crate::driver;

    let (control, handlers) = self.control.into_parts();
    let node = driver::create_checkbox(&CheckboxProps { control });

    if let Some(checked) = self.checked {
      checked.bind(move |checked| driver::set_checked(node, checked));
    }

    if let Some(action) = self.toggle_handler {
      driver::handle_toggle(node, action);
    }

    handlers.register(node);

    match self.label {
      Some(label) => crate::control::labeled(node, label),
      None => node,
    }
  }
}
