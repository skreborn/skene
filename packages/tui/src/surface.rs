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

use skene_core::{KeyEvent, Prevent};

use crate::{
  control::{FocusCore, StyleCore},
  props::Condition,
};

/// The control focus surface: autofocus, focus and anchor binding, and the
/// focus/blur/key handlers, forwarded to a control's [`FocusCore`].
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

  /// Sets the key-press handler, run for keys pressed while the control is focused;
  /// its [`Prevent`] verdict is honored per the dispatch contract.
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

/// The control styling surface: the static style and conditional variants, in the
/// control's own style vocabulary, forwarded to its [`StyleCore`].
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
