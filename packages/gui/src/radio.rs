//! One choice of a mutually-exclusive group.

use std::rc::Rc;

use skene_core::Source;

use crate::control::{ControlBase, control_surface};

/// A radio button: one choice of a mutually-exclusive group.
///
/// Members of one group share its name; the platform keeps one member checked and moves
/// between members with the arrow keys. [`bind_value`](Self::bind_value) wires a whole
/// group to one signal holding the selected value. Unstyled by default, with the
/// platform's full keyboard behavior.
#[must_use]
pub struct Radio {
  group: String,
  value: String,
  checked: Option<Source<bool>>,
  toggle_handler: Option<Rc<dyn Fn(bool)>>,
  control: ControlBase,
}

impl Radio {
  /// Creates the `group` member standing for `value`, unchecked.
  #[inline(always)]
  pub fn new(group: impl Into<String>, value: impl Into<String>) -> Self {
    Self {
      group: group.into(),
      value: value.into(),
      checked: None,
      toggle_handler: None,
      control: ControlBase::new(),
    }
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

  /// Sets the toggle handler, run with the new state when this member's state changes.
  #[inline(always)]
  pub fn handle_toggle(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.toggle_handler = Some(Rc::new(action));

    self
  }
}

control_surface!(Radio);

impl Radio {
  /// Binds the member to `signal` holding the group's selected value: the member shows
  /// as checked while the signal equals its value, and selecting it writes its value
  /// back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::Signal<String>) -> Self {
    use skene_reactive::{Get, Set};

    let value = self.value.clone();
    let selected = self.value.clone();

    self
      .derive_checked(move || signal.get() == value)
      .handle_toggle(move |checked| {
        if checked {
          signal.set(selected.clone());
        }
      })
  }
}

impl skene_core::View for Radio {
  fn build(self) -> skene_core::NodeId {
    use skene_core::RadioProps;

    use crate::driver;

    let (control, handlers) = self.control.into_parts();
    let node = driver::create_radio(&RadioProps {
      group: self.group,
      value: self.value,
      control,
    });

    if let Some(checked) = self.checked {
      checked.bind(move |checked| driver::set_checked(node, checked));
    }

    if let Some(action) = self.toggle_handler {
      driver::handle_toggle(node, action);
    }

    handlers.register(node);

    node
  }
}
