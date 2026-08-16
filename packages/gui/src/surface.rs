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

use std::rc::Rc;

use skene_core::{Animation, BoxStyle, Condition, KeyEvent, PointerEvent, ScrollEvent, Transition};

use crate::control::{DecorationCore, EventCore, FocusCore};

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
}

/// The drawn-control event surface: every event a control can carry, forwarded to its
/// [`EventCore`].
///
/// Each control offers the whole set whether or not it acts on a given event, so a
/// handler always goes on the control itself rather than on a wrapper around it.
///
/// A handler observes and nothing more. A control owns its own behavior and consumes
/// the events it acts on—what the platform and the ancestors are told is its decision,
/// made once, where the behavior is—so nothing here returns a verdict.
pub trait HasEvents: Sized {
  /// The core the handlers land on.
  fn event_core(&mut self) -> &mut EventCore;

  /// Sets the click handler.
  #[inline(always)]
  #[must_use]
  fn handle_click(mut self, action: impl Fn() + 'static) -> Self {
    self.event_core().click = Some(Rc::new(move |&()| action()));

    self
  }

  /// Sets the key-press handler.
  #[inline(always)]
  #[must_use]
  fn handle_key_down(mut self, action: impl Fn(&KeyEvent) + 'static) -> Self {
    self.event_core().key_down = Some(Rc::new(action));

    self
  }

  /// Sets the pointer-press handler.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_down(mut self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self.event_core().pointer_down = Some(Rc::new(action));

    self
  }

  /// Sets the pointer-move handler.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_move(mut self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self.event_core().pointer_move = Some(Rc::new(action));

    self
  }

  /// Sets the pointer-release handler.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_up(mut self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self.event_core().pointer_up = Some(Rc::new(action));

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

/// The control decoration surface: a control's own chrome, spelled per property exactly
/// as [`Container`](crate::Container) spells it, forwarded to a control's
/// [`DecorationCore`].
///
/// A control that draws nothing needs none of this—its chrome is the `Container` its
/// child is made of. This is for the controls that decorate an element of their own: a
/// text field's box, a button's, and the browser's own controls, which a component
/// library styles to sit as close to its drawn counterpart as an engine allows.
pub trait HasDecoration: Sized {
  /// The core the decoration lands on.
  fn decoration_core(&mut self) -> &mut DecorationCore;

  /// Sets the background color.
  #[inline(always)]
  #[must_use]
  fn set_color(mut self, color: skene_core::Color) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_color(color);

    self
  }

  /// Sets the background color applied while `condition` holds.
  #[inline(always)]
  #[must_use]
  fn set_color_when(mut self, condition: Condition, color: skene_core::Color) -> Self {
    self
      .decoration_core()
      .variants
      .push((condition, BoxStyle::new().set_color(color)));

    self
  }

  /// Sets the border.
  #[inline(always)]
  #[must_use]
  fn set_border(mut self, border: skene_core::Border) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_border(border);

    self
  }

  /// Sets the border applied while `condition` holds.
  #[inline(always)]
  #[must_use]
  fn set_border_when(mut self, condition: Condition, border: skene_core::Border) -> Self {
    self
      .decoration_core()
      .variants
      .push((condition, BoxStyle::new().set_border(border)));

    self
  }

  /// Sets the corner radius.
  #[inline(always)]
  #[must_use]
  fn set_border_radius(mut self, border_radius: f32) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_border_radius(border_radius);

    self
  }

  /// Sets the corner radius applied while `condition` holds.
  #[inline(always)]
  #[must_use]
  fn set_border_radius_when(mut self, condition: Condition, border_radius: f32) -> Self {
    self
      .decoration_core()
      .variants
      .push((condition, BoxStyle::new().set_border_radius(border_radius)));

    self
  }

  /// Sets the padding inside the control's own box.
  #[inline(always)]
  #[must_use]
  fn set_padding(mut self, padding: crate::Inset) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_padding(padding);

    self
  }

  /// Sets the pointer shape shown over the control.
  #[inline(always)]
  #[must_use]
  fn set_cursor(mut self, cursor: skene_core::MouseCursor) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_cursor(cursor);

    self
  }

  /// Sets a static width.
  #[inline(always)]
  #[must_use]
  fn set_width(mut self, width: impl Into<skene_core::Dimension<f32>>) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_width(width);

    self
  }

  /// Sets a static height.
  #[inline(always)]
  #[must_use]
  fn set_height(mut self, height: impl Into<skene_core::Dimension<f32>>) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_height(height);

    self
  }

  /// Sets a static minimum width.
  #[inline(always)]
  #[must_use]
  fn set_min_width(mut self, min_width: impl Into<skene_core::Dimension<f32>>) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_min_width(min_width);

    self
  }

  /// Sets a static maximum width.
  #[inline(always)]
  #[must_use]
  fn set_max_width(mut self, max_width: impl Into<skene_core::Dimension<f32>>) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_max_width(max_width);

    self
  }

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  #[must_use]
  fn set_transition(mut self, transition: Transition) -> Self {
    self.decoration_core().transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  #[must_use]
  fn set_animation(mut self, animation: Animation<BoxStyle>) -> Self {
    self.decoration_core().animations.push(animation);

    self
  }
}
