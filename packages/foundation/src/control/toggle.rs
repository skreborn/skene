//! A checkbox's behavior.

use std::{cell::Cell, rc::Rc};

use skene_reactive::Signal;

use super::{Behavior, ControlHandle, Controlled};
use crate::{
  AccessProp,
  CheckState,
  ControlState,
  Controller,
  EventKind,
  KeyEvent,
  NativeElement,
  Prevent,
  Role,
  contents,
};

/// A checkbox: a click, `Space` or the platform's own toggle flips it.
///
/// `Space` flips it once, as it is released, when the checkbox has held focus since the
/// press.
///
/// Controlled: the checked state it reports is what
/// [`derive_checked`](Self::derive_checked) reads, and a flip only calls
/// [`handle_toggle`](Self::handle_toggle) with the state it asks for. With no handler
/// nothing changes; [`bind_checked`](Self::bind_checked) wires both to one signal. The
/// state crosses to the platform as its [`Checked`](crate::Content::Checked) content,
/// which a [`Controller`] holds: a flip the platform reports applied asks for the state
/// it shows, and one the truth then refuses is undone.
///
/// [`switch`](Self::switch) makes it a switch: the same interaction, reported as
/// [`Role::Switch`], on or off rather than checked.
#[derive(Default)]
#[must_use]
pub struct Toggle {
  checked: Controlled<bool>,
  armed: Cell<bool>,
  switch: bool,
}

impl std::fmt::Debug for Toggle {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Toggle").finish_non_exhaustive()
  }
}

impl Toggle {
  /// The events a checkbox acts on: a click, the keys that flip it and the blur that
  /// disarms a `Space` press.
  const EVENTS: &'static [EventKind] = &[
    EventKind::Click,
    EventKind::KeyDown,
    EventKind::KeyUp,
    EventKind::Blur,
  ];

  /// An unchecked checkbox with no handler.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// An unchecked switch with no handler: a checkbox reported as [`Role::Switch`].
  #[inline(always)]
  pub fn switch() -> Self {
    Self {
      switch: true,
      ..Self::default()
    }
  }

  /// Whether it is a switch rather than a checkbox.
  #[inline(always)]
  #[must_use]
  pub const fn is_switch(&self) -> bool {
    self.switch
  }

  /// The closure the checked state is read through; `false` with none set.
  #[must_use]
  pub fn reads_checked(&self) -> Rc<dyn Fn() -> bool> {
    self.checked.reader().unwrap_or_else(|| Rc::new(|| false))
  }

  /// Sets the checked state.
  #[inline(always)]
  pub fn set_checked(mut self, checked: bool) -> Self {
    self.checked.set(checked);

    self
  }

  /// Derives the checked state from the signals `checked` reads.
  pub fn derive_checked(mut self, checked: impl Fn() -> bool + 'static) -> Self {
    self.checked.derive(checked);

    self
  }

  /// Runs `action` with the state a flip asks for.
  pub fn handle_toggle(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.checked.handle(action);

    self
  }

  /// Binds the checked state to `signal` in both directions.
  #[inline(always)]
  pub fn bind_checked(mut self, signal: Signal<bool>) -> Self {
    self.checked.bind(signal);

    self
  }

  /// The checked state currently reported.
  #[inline(always)]
  fn read(&self) -> bool {
    self.checked.read().unwrap_or(false)
  }

  /// Asks for `checked`.
  #[inline(always)]
  fn ask(&self, checked: bool) {
    self.checked.ask(checked);
  }

  /// Asks for the opposite state.
  #[inline(always)]
  fn flip(&self) {
    self.ask(!self.read());
  }
}

impl Behavior for Toggle {
  #[inline(always)]
  fn native(&self) -> Option<NativeElement> {
    Some(NativeElement::Checkbox)
  }

  #[inline(always)]
  fn role(&self) -> Option<Role> {
    Some(if self.switch {
      Role::Switch
    }
    else {
      Role::CheckBox
    })
  }

  #[inline(always)]
  fn listens(&self) -> &'static [EventKind] {
    Self::EVENTS
  }

  fn attach(&self, control: &ControlHandle) {
    let checked = self.reads_checked();
    let asks = self.checked.clone();

    control.register(Controller::new(
      contents::CHECKED,
      move || CheckState::from(checked()),
      move |next| asks.ask(*next == CheckState::On),
    ));
  }

  fn on_click(&self, _control: &ControlHandle) -> Prevent {
    self.flip();

    Prevent::Bubbling
  }

  fn on_key(&self, event: &KeyEvent, _control: &ControlHandle) -> Prevent {
    if !event.types(" ") {
      return Prevent::None;
    }

    if !event.repeat {
      self.armed.set(true);
    }

    Prevent::All
  }

  fn on_key_up(&self, event: &KeyEvent, _control: &ControlHandle) -> Prevent {
    if !event.types(" ") || !self.armed.replace(false) {
      return Prevent::None;
    }

    self.flip();

    Prevent::All
  }

  #[inline(always)]
  fn on_blur(&self, _control: &ControlHandle) {
    self.armed.set(false);
  }

  fn access(&self) -> Vec<AccessProp> {
    vec![AccessProp::Checked(self.read().into())]
  }

  fn state(&self) -> ControlState {
    ControlState {
      checked: self.read().into(),
      ..ControlState::DEFAULT
    }
  }
}

#[cfg(test)]
mod tests {
  use super::Toggle;
  use crate::EventKind;

  /// A checkbox listens to the click, the keys and the blur it acts on, and nothing
  /// else; the platform's own flip is a report of its content.
  #[test]
  fn a_checkbox_listens_to_what_flips_it() {
    assert_eq!(
      Toggle::EVENTS,
      [
        EventKind::Click,
        EventKind::KeyDown,
        EventKind::KeyUp,
        EventKind::Blur,
      ]
    );
  }
}
