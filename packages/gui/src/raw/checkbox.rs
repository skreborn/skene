//! A two-state toggle a view draws itself.

use std::{ops::ControlFlow, rc::Rc};

use skene_core::{
  AccessProp,
  AnyView,
  FocusOrder,
  IntoView,
  KeyEvent,
  NodeId,
  Override,
  Prevent,
  Role,
  Source,
  View,
};

use crate::Container;

/// A checkbox drawn by the view it holds: the control supplies the behavior, the child
/// supplies every pixel.
///
/// Keyboard activation, the tab order, the checkbox role, and the checked state it
/// reports come from the control; nothing is drawn for you, so the child renders both
/// states—derive it from the same signal the checkbox is bound to. `Space` and a
/// pointer click toggle, matching the browser's own; both are overridable, since a
/// handler installed with [`handle_click`](Self::handle_click) or
/// [`handle_key_down`](Self::handle_key_down) runs ahead of them and may take the
/// event.
///
/// The state flows both ways: [`derive_checked`](Self::derive_checked) (or
/// [`set_checked`](Self::set_checked)) drives what is reported, and
/// [`handle_toggle`](Self::handle_toggle) receives each change—
/// [`bind_checked`](Self::bind_checked) wires both to one signal.
///
/// Renders the same on every GUI platform. Where the browser's own chrome and its form
/// participation are wanted instead, [`WebCheckbox`](crate::WebCheckbox) is the other
/// half of the pair.
#[must_use]
pub struct RawCheckbox<C = skene_core::NoChild> {
  child: C,
  checked: Option<Source<bool>>,
  toggle_handler: Option<Rc<dyn Fn(bool)>>,
  click: Override<()>,
  key_down: Override<KeyEvent>,
  focus_order: FocusOrder,
  label: Option<String>,
}

impl RawCheckbox {
  /// Creates an unchecked checkbox, drawn by the child it is given.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      child: skene_core::NoChild,
      checked: None,
      toggle_handler: None,
      click: Override::new(),
      key_down: Override::new(),
      focus_order: FocusOrder::Auto,
      label: None,
    }
  }
}

impl Default for RawCheckbox {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<C> RawCheckbox<C> {
  /// Sets the child that draws the control, which it requires before it can build.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView) -> RawCheckbox<AnyView> {
    RawCheckbox {
      child: child.into_view(),
      checked: self.checked,
      toggle_handler: self.toggle_handler,
      click: self.click,
      key_down: self.key_down,
      focus_order: self.focus_order,
      label: self.label,
    }
  }

  /// Sets a static checked state.
  #[inline(always)]
  pub fn set_checked(mut self, checked: bool) -> Self {
    self.checked = Some(Source::Static(checked));

    self
  }

  /// Sets a derived checked state, re-reported whenever it changes.
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

  /// Binds the state to `signal` in both directions: the checkbox reports the signal
  /// and each toggle writes it back.
  #[inline(always)]
  pub fn bind_checked(self, signal: skene_reactive::Signal<bool>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_checked(move || signal.get())
      .handle_toggle(move |checked| signal.set(checked))
  }

  /// Installs a click handler ahead of the checkbox's own toggle.
  ///
  /// [`Continue`](ControlFlow::Continue) leaves the toggle to run as though nothing
  /// were installed; [`Break`](ControlFlow::Break) takes the click, and the checkbox
  /// does nothing further.
  #[inline(always)]
  pub fn handle_click(mut self, action: impl Fn() -> ControlFlow<Prevent> + 'static) -> Self {
    self.click.set(move |&()| action());

    self
  }

  /// Installs a key handler ahead of the checkbox's own `Space` toggle.
  ///
  /// [`Continue`](ControlFlow::Continue) leaves the toggle to run as though nothing
  /// were installed; [`Break`](ControlFlow::Break) takes the key, and the checkbox does
  /// nothing further.
  #[inline(always)]
  pub fn handle_key_down(
    mut self,
    action: impl Fn(&KeyEvent) -> ControlFlow<Prevent> + 'static,
  ) -> Self {
    self.key_down.set(action);

    self
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

impl View for RawCheckbox<AnyView> {
  fn build(self) -> NodeId {
    let checked = self.checked;
    let label = self.label;
    let state = {
      let checked = checked.clone();
      let label = label.clone();

      move || {
        let mut state = vec![AccessProp::Checked(read_checked(checked.as_ref()).into())];

        if let Some(label) = &label {
          state.push(AccessProp::Label(label.clone()));
        }

        state
      }
    };
    let toggle: Rc<dyn Fn()> = {
      let checked = checked.clone();
      let handler = self.toggle_handler.clone();

      Rc::new(move || {
        if let Some(action) = &handler {
          action(!read_checked(checked.as_ref()));
        }
      })
    };
    let click = {
      let toggle = Rc::clone(&toggle);

      self.click.wrap(move |&()| {
        toggle();

        Prevent::None
      })
    };
    // `Space` activates a checkbox and `Enter` does not, which is what the browser's
    // own does; preventing the platform's response keeps the page from scrolling under
    // it.
    let key = self.key_down.wrap(move |event: &KeyEvent| {
      if event.key == " " {
        toggle();

        return Prevent::Platform;
      }

      Prevent::None
    });

    Container::new()
      .set_role(Role::Checkbox)
      .set_focus_order(self.focus_order)
      .derive_accessibility(state)
      .handle_click(move || click(&()))
      .handle_key_down(key)
      .set_child(self.child)
      .build()
  }
}

/// The state a checked source currently reports; an unset one reads as unchecked.
#[inline(always)]
fn read_checked(checked: Option<&Source<bool>>) -> bool {
  checked.is_some_and(Source::get)
}
