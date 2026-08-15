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

use skene_core::{
  Animation,
  BoxStyle,
  Condition,
  KeyEvent,
  PointerEvent,
  Prevent,
  ScrollEvent,
  Transition,
};

use crate::control::{EventCore, FocusCore, StyleCore};

/// The control focus surface: focus order, autofocus, focus and anchor binding, and
/// the focus/blur/key handlers, forwarded to a control's [`FocusCore`].
pub trait HasFocus: Sized {
  /// The core the focus behavior lands on.
  fn focus_core(&mut self) -> &mut FocusCore;

  /// Sets the control's place in sequential focus traversal.
  #[inline(always)]
  #[must_use]
  fn set_focus_order(mut self, focus_order: skene_core::FocusOrder) -> Self {
    self.focus_core().focus_order = Some(focus_order);

    self
  }

  /// Makes the control take focus when the page loads.
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

  /// Sets the focus handler.
  #[inline(always)]
  #[must_use]
  fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.focus_core().focus_handler = Some(Rc::new(action));

    self
  }

  /// Sets the blur handler.
  #[inline(always)]
  #[must_use]
  fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.focus_core().blur_handler = Some(Rc::new(action));

    self
  }

  /// Sets the key-press handler.
  #[inline(always)]
  #[must_use]
  fn handle_key_down<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&KeyEvent) -> R + 'static,
  ) -> Self {
    self.focus_core().key_down_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }
}

/// The drawn-control event surface: every event a control can carry, forwarded to its
/// [`EventCore`].
///
/// Each control offers the whole set whether or not it acts on a given event, so a
/// handler always goes on the control itself rather than on a wrapper around it, and a
/// control that later grows behavior for an event does not change the signature its
/// users already wrote against.
///
/// The preventable handlers return [`ControlFlow`]: `Continue` leaves the control to act
/// as though nothing were installed, and `Break` takes the event, carrying the verdict
/// the dispatch goes on to see. The notification handlers—pointer enter and leave,
/// focus, blur, scroll—report what already happened and take no verdict, on a control
/// exactly as on a [`Container`](crate::Container).
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

  /// Installs a pointer-press handler ahead of the control's own behavior.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_down(
    mut self,
    action: impl Fn(&PointerEvent) -> ControlFlow<Prevent> + 'static,
  ) -> Self {
    self.event_core().pointer_down.set(action);

    self
  }

  /// Installs a pointer-move handler ahead of the control's own behavior.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_move(
    mut self,
    action: impl Fn(&PointerEvent) -> ControlFlow<Prevent> + 'static,
  ) -> Self {
    self.event_core().pointer_move.set(action);

    self
  }

  /// Installs a pointer-release handler ahead of the control's own behavior.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_up(
    mut self,
    action: impl Fn(&PointerEvent) -> ControlFlow<Prevent> + 'static,
  ) -> Self {
    self.event_core().pointer_up.set(action);

    self
  }

  /// Sets the pointer-enter handler.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self.event_core().pointer_enter = Some(Rc::new(action));

    self
  }

  /// Sets the pointer-leave handler.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self.event_core().pointer_leave = Some(Rc::new(action));

    self
  }

  /// Sets the focus handler.
  #[inline(always)]
  #[must_use]
  fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.event_core().focus = Some(Rc::new(action));

    self
  }

  /// Sets the blur handler.
  #[inline(always)]
  #[must_use]
  fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.event_core().blur = Some(Rc::new(action));

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

/// The control styling surface: the static style, conditional variants, transitions,
/// and animations, forwarded to a control's [`StyleCore`].
pub trait HasStyle: Sized {
  /// The core the styling lands on.
  fn style_core(&mut self) -> &mut StyleCore;

  /// Sets the static style.
  #[inline(always)]
  #[must_use]
  fn set_style(mut self, style: BoxStyle) -> Self {
    self.style_core().style = style;

    self
  }

  /// Adds a conditional style variant, applied while `condition` holds.
  #[inline(always)]
  #[must_use]
  fn set_when(mut self, condition: Condition, style: BoxStyle) -> Self {
    self.style_core().variants.push((condition, style));

    self
  }

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  #[must_use]
  fn set_transition(mut self, transition: Transition) -> Self {
    self.style_core().transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  #[must_use]
  fn set_animation(mut self, animation: Animation<BoxStyle>) -> Self {
    self.style_core().animations.push(animation);

    self
  }
}
