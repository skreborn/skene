//! The shared control surfaces, as traits over an accessor to a control's cores.
//!
//! Each requires one accessor to the state it writes and provides every builder method
//! on top, so a view wrapping a control—as every `Simple*` wraps its `Raw*`—implements
//! the accessor in a line and offers the whole vocabulary, exactly as
//! [`HasChildren`](skene_core::HasChildren) provides `set_children` over
//! `set_children_to`.
//!
//! The box surface has no counterpart here: only [`Container`](crate::Container)
//! decorates, so there is nothing to forward.

use std::{ops::ControlFlow, rc::Rc};

use skene_core::{KeyEvent, Prevent};

use crate::{
  control::{EventCore, FocusCore, StyleCore},
  events::ScrollEvent,
  props::Condition,
};

/// The control focus surface: autofocus and the focus and anchor bindings, forwarded to
/// a control's [`FocusCore`].
///
/// The handlers live on [`HasEvents`], since a control's own behavior is something a
/// handler may take over and a focus binding is not.
pub trait HasFocus: Sized {
  /// The core the focus behavior lands on.
  fn focus_core(&mut self) -> &mut FocusCore;

  /// Makes the control take focus when it mounts.
  #[inline(always)]
  #[must_use]
  fn set_autofocus(mut self) -> Self {
    self.focus_core().autofocus = true;

    self
  }

  /// Binds `handle` to the control, so
  /// [`FocusHandle::focus`](crate::FocusHandle::focus) targets it once built.
  #[inline(always)]
  #[must_use]
  fn bind_focus(mut self, handle: &crate::FocusHandle) -> Self {
    self.focus_core().focus_handle = Some(handle.clone());

    self
  }

  /// Binds `anchor` to the control, so an overlay's
  /// [`set_anchor`](crate::Overlay::set_anchor) positions against it once built.
  #[inline(always)]
  #[must_use]
  fn bind_anchor(mut self, anchor: &crate::Anchor) -> Self {
    self.focus_core().anchor = Some(anchor.clone());

    self
  }
}

/// The terminal-control event surface: every event a control can carry, forwarded to its
/// [`EventCore`].
///
/// Each control offers the whole set whether or not it acts on a given event, so a
/// handler always goes on the control itself, and a control that later grows behavior
/// for an event does not change the signature its users already wrote against.
///
/// The preventable handlers return [`ControlFlow`]: `Continue` leaves the control to act
/// as though nothing were installed, and `Break` takes the event, carrying the verdict
/// the dispatch goes on to see. A control the driver realizes acts on nothing itself—its
/// behavior is the terminal's, which [`Prevent::Platform`] suppresses.
pub trait HasEvents: Sized {
  /// The core the handlers land on.
  fn event_core(&mut self) -> &mut EventCore;

  /// Installs a click handler ahead of the control's own behavior.
  #[inline(always)]
  #[must_use]
  fn handle_click(mut self, action: impl Fn() -> ControlFlow<Prevent> + 'static) -> Self {
    self.event_core().click.set(move |&()| action());

    self
  }

  /// Installs a key handler ahead of the control's own behavior.
  #[inline(always)]
  #[must_use]
  fn handle_key_down(
    mut self,
    action: impl Fn(&KeyEvent) -> ControlFlow<Prevent> + 'static,
  ) -> Self {
    self.event_core().key_down.set(action);

    self
  }

  /// Sets the pointer-enter handler.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self.event_core().pointer_enter = Some(Rc::new(move |&()| action()));

    self
  }

  /// Sets the pointer-leave handler.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self.event_core().pointer_leave = Some(Rc::new(move |&()| action()));

    self
  }

  /// Sets the focus handler.
  #[inline(always)]
  #[must_use]
  fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.event_core().focus = Some(Rc::new(move |&()| action()));

    self
  }

  /// Sets the blur handler.
  #[inline(always)]
  #[must_use]
  fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.event_core().blur = Some(Rc::new(move |&()| action()));

    self
  }

  /// Sets the scroll handler.
  #[inline(always)]
  #[must_use]
  fn handle_scroll(mut self, action: impl Fn(&ScrollEvent) + 'static) -> Self {
    self.event_core().scroll = Some(Rc::new(action));

    self
  }
}

/// The control styling surface: the static style and its conditional variants,
/// forwarded to a control's [`StyleCore`].
pub trait HasStyle: Sized {
  /// The style vocabulary the control accepts.
  type Style;

  /// The core the styling lands on.
  fn style_core(&mut self) -> &mut StyleCore<Self::Style>;

  /// Sets the static style.
  #[inline(always)]
  #[must_use]
  fn set_style(mut self, style: Self::Style) -> Self {
    self.style_core().style = style;

    self
  }

  /// Adds a conditional style variant, applied while `condition` holds.
  #[inline(always)]
  #[must_use]
  fn set_when(mut self, condition: Condition, style: Self::Style) -> Self {
    self.style_core().variants.push((condition, style));

    self
  }
}
