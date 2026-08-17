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

use skene_core::KeyEvent;

use crate::{
  control::{DecorationCore, EventCore, FocusCore, MarkCore},
  events::ScrollEvent,
  props::{BoxStyle, Condition, ControlStyle},
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
/// A handler observes and nothing more. A control owns its own behavior and consumes
/// the events it acts on—what the terminal and the ancestors are told is its decision,
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

/// The mark decoration surface: what a terminal control draws its marks with, spelled
/// per property, forwarded to a control's [`MarkCore`].
///
/// A terminal draws a control's box itself, so what a control decorates is its marks—
/// the check, the dot, the thumb, the indicator—rather than a surface.
pub trait HasMarks: Sized {
  /// The core the decoration lands on.
  fn mark_core(&mut self) -> &mut MarkCore;

  /// Sets the marks' color.
  #[inline(always)]
  #[must_use]
  fn set_color(mut self, color: skene_core::Color) -> Self {
    let core = self.mark_core();

    core.style = core.style.set_color(color);

    self
  }

  /// Sets the marks' color while `condition` holds.
  #[inline(always)]
  #[must_use]
  fn set_color_when(mut self, condition: Condition, color: skene_core::Color) -> Self {
    self
      .mark_core()
      .variants
      .push((condition, ControlStyle::new().set_color(color)));

    self
  }

  /// Sets whether the marks are bold.
  #[inline(always)]
  #[must_use]
  fn set_bold(mut self, bold: bool) -> Self {
    let core = self.mark_core();

    core.style = core.style.set_bold(bold);

    self
  }

  /// Sets whether the marks are bold while `condition` holds.
  #[inline(always)]
  #[must_use]
  fn set_bold_when(mut self, condition: Condition, bold: bool) -> Self {
    self
      .mark_core()
      .variants
      .push((condition, ControlStyle::new().set_bold(bold)));

    self
  }

  /// Sets the pointer shape shown over the control, in terminals that honor it.
  #[inline(always)]
  #[must_use]
  fn set_cursor(mut self, cursor: skene_core::MouseCursor) -> Self {
    let core = self.mark_core();

    core.style = core.style.set_cursor(cursor);

    self
  }
}

/// The box decoration surface: a terminal control that owns a box of its own, spelled
/// per property, forwarded to its [`DecorationCore`].
pub trait HasDecoration: Sized {
  /// The core the decoration lands on.
  fn decoration_core(&mut self) -> &mut DecorationCore;

  /// Sets the background color, or an [`Adaptive`](skene_core::Adaptive) one: a token's
  /// own conditional variants land beside the property, exactly as `set_color_when`
  /// would spell them.
  #[inline(always)]
  #[must_use]
  fn set_color(mut self, color: impl Into<crate::props::Adaptive<skene_core::Color>>) -> Self {
    let (base, variants) = color.into().parts();
    let core = self.decoration_core();

    core.style = core.style.clone().set_color(base);

    for (condition, color) in variants {
      core
        .variants
        .push((condition, BoxStyle::new().set_color(color)));
    }

    self
  }

  /// Sets the background color applied while `condition` holds. An
  /// [`Adaptive`](crate::props::Adaptive) color keeps its own conditions, narrowed to
  /// also require `condition`.
  #[inline(always)]
  #[must_use]
  fn set_color_when(
    mut self,
    condition: Condition,
    color: impl Into<crate::props::Adaptive<skene_core::Color>>,
  ) -> Self {
    let core = self.decoration_core();

    for (condition, color) in color.into().narrowed(condition) {
      core
        .variants
        .push((condition, BoxStyle::new().set_color(color)));
    }

    self
  }

  /// Sets the border, or an [`Adaptive`](skene_core::Adaptive) one.
  #[inline(always)]
  #[must_use]
  fn set_border(mut self, border: impl Into<crate::props::Adaptive<skene_core::Color>>) -> Self {
    let (base, variants) = border.into().parts();
    let core = self.decoration_core();

    core.style = core.style.clone().set_border(base);

    for (condition, border) in variants {
      core
        .variants
        .push((condition, BoxStyle::new().set_border(border)));
    }

    self
  }

  /// Sets the border applied while `condition` holds. An
  /// [`Adaptive`](crate::props::Adaptive) border keeps its own conditions, narrowed to
  /// also require `condition`.
  #[inline(always)]
  #[must_use]
  fn set_border_when(
    mut self,
    condition: Condition,
    border: impl Into<crate::props::Adaptive<skene_core::Color>>,
  ) -> Self {
    let core = self.decoration_core();

    for (condition, border) in border.into().narrowed(condition) {
      core
        .variants
        .push((condition, BoxStyle::new().set_border(border)));
    }

    self
  }

  /// Sets the padding inside the control's own box, in cells.
  #[inline(always)]
  #[must_use]
  fn set_padding(mut self, padding: crate::Inset) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_padding(padding);

    self
  }

  /// Sets a static width, in cells.
  #[inline(always)]
  #[must_use]
  fn set_width(mut self, width: impl Into<skene_core::Dimension<u32>>) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_width(width);

    self
  }

  /// Sets a static height, in cells.
  #[inline(always)]
  #[must_use]
  fn set_height(mut self, height: impl Into<skene_core::Dimension<u32>>) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_height(height);

    self
  }

  /// Sets the glyphs the border is drawn with.
  #[inline(always)]
  #[must_use]
  fn set_border_glyphs(mut self, glyphs: crate::props::BorderGlyphs) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_border_glyphs(glyphs);

    self
  }

  /// Sets the pointer shape shown over the control, in terminals that honor it.
  #[inline(always)]
  #[must_use]
  fn set_cursor(mut self, cursor: skene_core::MouseCursor) -> Self {
    let core = self.decoration_core();

    core.style = core.style.clone().set_cursor(cursor);

    self
  }
}
