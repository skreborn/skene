//! The shared builder surfaces, as traits over an accessor: sizing, decoration, and
//! events land on a [`StyledBox`]; focus behavior and styling land on a control's
//! cores.
//!
//! Each trait requires one accessor to the state it writes and provides every builder
//! method on top, so a view wrapping a [`StyledBox`]—or wrapping another view that
//! carries the surface—implements the accessor in a line or two and offers the whole
//! vocabulary, exactly as [`HasChildren`](skene_core::HasChildren) provides
//! `set_children` over `set_children_to`.

use std::rc::Rc;

use skene_core::{
  Animation,
  Border,
  BoxStyle,
  Color,
  Condition,
  Dimension,
  KeyEvent,
  MouseCursor,
  Overflow,
  PointerEvent,
  Prevent,
  Role,
  ScrollEvent,
  Source,
  Transition,
};

use crate::{
  Inset,
  StyledBox,
  control::{FocusCore, StyleCore},
};

/// The sizing surface: dimensions and their limits, forwarded to a wrapped
/// [`StyledBox`].
pub trait HasSize: Sized {
  /// The box the sizing lands on.
  fn size_box(&mut self) -> &mut StyledBox;

  /// Sets a static width: logical pixels, a percentage, or auto.
  #[inline(always)]
  #[must_use]
  fn set_width(mut self, width: impl Into<Dimension<f32>>) -> Self {
    self.size_box().width = Some(Source::Static(width.into()));

    self
  }

  /// Sets a derived width.
  #[inline(always)]
  #[must_use]
  fn derive_width<D: Into<Dimension<f32>>>(mut self, width: impl Fn() -> D + 'static) -> Self {
    self.size_box().width = Some(Source::Derived(Rc::new(move || width().into())));

    self
  }

  /// Sets a static height: logical pixels, a percentage, or auto.
  #[inline(always)]
  #[must_use]
  fn set_height(mut self, height: impl Into<Dimension<f32>>) -> Self {
    self.size_box().height = Some(Source::Static(height.into()));

    self
  }

  /// Sets a derived height.
  #[inline(always)]
  #[must_use]
  fn derive_height<D: Into<Dimension<f32>>>(mut self, height: impl Fn() -> D + 'static) -> Self {
    self.size_box().height = Some(Source::Derived(Rc::new(move || height().into())));

    self
  }

  /// Sets a static minimum width: logical pixels, a percentage, or auto.
  #[inline(always)]
  #[must_use]
  fn set_min_width(mut self, min_width: impl Into<Dimension<f32>>) -> Self {
    self.size_box().min_width = Some(Source::Static(min_width.into()));

    self
  }

  /// Sets a derived minimum width.
  #[inline(always)]
  #[must_use]
  fn derive_min_width<D: Into<Dimension<f32>>>(
    mut self,
    min_width: impl Fn() -> D + 'static,
  ) -> Self {
    self.size_box().min_width = Some(Source::Derived(Rc::new(move || min_width().into())));

    self
  }

  /// Sets a static minimum height: logical pixels, a percentage, or auto.
  #[inline(always)]
  #[must_use]
  fn set_min_height(mut self, min_height: impl Into<Dimension<f32>>) -> Self {
    self.size_box().min_height = Some(Source::Static(min_height.into()));

    self
  }

  /// Sets a derived minimum height.
  #[inline(always)]
  #[must_use]
  fn derive_min_height<D: Into<Dimension<f32>>>(
    mut self,
    min_height: impl Fn() -> D + 'static,
  ) -> Self {
    self.size_box().min_height = Some(Source::Derived(Rc::new(move || min_height().into())));

    self
  }

  /// Sets a static maximum width: logical pixels, a percentage, or auto.
  #[inline(always)]
  #[must_use]
  fn set_max_width(mut self, max_width: impl Into<Dimension<f32>>) -> Self {
    self.size_box().max_width = Some(Source::Static(max_width.into()));

    self
  }

  /// Sets a derived maximum width.
  #[inline(always)]
  #[must_use]
  fn derive_max_width<D: Into<Dimension<f32>>>(
    mut self,
    max_width: impl Fn() -> D + 'static,
  ) -> Self {
    self.size_box().max_width = Some(Source::Derived(Rc::new(move || max_width().into())));

    self
  }

  /// Sets a static maximum height: logical pixels, a percentage, or auto.
  #[inline(always)]
  #[must_use]
  fn set_max_height(mut self, max_height: impl Into<Dimension<f32>>) -> Self {
    self.size_box().max_height = Some(Source::Static(max_height.into()));

    self
  }

  /// Sets a derived maximum height.
  #[inline(always)]
  #[must_use]
  fn derive_max_height<D: Into<Dimension<f32>>>(
    mut self,
    max_height: impl Fn() -> D + 'static,
  ) -> Self {
    self.size_box().max_height = Some(Source::Derived(Rc::new(move || max_height().into())));

    self
  }

  /// Sets a static width-to-height ratio the unsized axis follows.
  #[inline(always)]
  #[must_use]
  fn set_aspect_ratio(mut self, aspect_ratio: f32) -> Self {
    self.size_box().aspect_ratio = Some(Source::Static(aspect_ratio));

    self
  }

  /// Sets a derived width-to-height ratio the unsized axis follows.
  #[inline(always)]
  #[must_use]
  fn derive_aspect_ratio(mut self, aspect_ratio: impl Fn() -> f32 + 'static) -> Self {
    self.size_box().aspect_ratio = Some(Source::Derived(Rc::new(aspect_ratio)));

    self
  }
}

/// The decoration surface: spacing, colors, borders, cursor, overflow, the semantic
/// role, style variants, transitions, animations, and anchor binding, forwarded to a
/// wrapped [`StyledBox`].
pub trait HasDecoration: Sized {
  /// The box the decoration lands on.
  fn decoration_box(&mut self) -> &mut StyledBox;

  /// Sets a static padding.
  #[inline(always)]
  #[must_use]
  fn set_padding(mut self, padding: Inset) -> Self {
    self.decoration_box().padding = Some(Source::Static(padding));

    self
  }

  /// Sets a derived padding.
  #[inline(always)]
  #[must_use]
  fn derive_padding(mut self, padding: impl Fn() -> Inset + 'static) -> Self {
    self.decoration_box().padding = Some(Source::Derived(Rc::new(padding)));

    self
  }

  /// Sets a static margin on each edge.
  #[inline(always)]
  #[must_use]
  fn set_margin(mut self, margin: skene_core::Inset<Dimension<f32>>) -> Self {
    self.decoration_box().margin = Some(Source::Static(margin));

    self
  }

  /// Sets a derived margin on each edge.
  #[inline(always)]
  #[must_use]
  fn derive_margin(
    mut self,
    margin: impl Fn() -> skene_core::Inset<Dimension<f32>> + 'static,
  ) -> Self {
    self.decoration_box().margin = Some(Source::Derived(Rc::new(margin)));

    self
  }

  /// Sets a static in-flow shift from each edge.
  #[inline(always)]
  #[must_use]
  fn set_offset(mut self, offset: skene_core::Inset<Dimension<f32>>) -> Self {
    self.decoration_box().offset = Some(Source::Static(offset));

    self
  }

  /// Sets a derived in-flow shift.
  #[inline(always)]
  #[must_use]
  fn derive_offset(
    mut self,
    offset: impl Fn() -> skene_core::Inset<Dimension<f32>> + 'static,
  ) -> Self {
    self.decoration_box().offset = Some(Source::Derived(Rc::new(offset)));

    self
  }

  /// Sets a static background color.
  #[inline(always)]
  #[must_use]
  fn set_color(mut self, color: Color) -> Self {
    self.decoration_box().color = Some(Source::Static(color));

    self
  }

  /// Sets a derived background color.
  #[inline(always)]
  #[must_use]
  fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.decoration_box().color = Some(Source::Derived(Rc::new(color)));

    self
  }

  /// Sets a static border.
  #[inline(always)]
  #[must_use]
  fn set_border(mut self, border: Border) -> Self {
    self.decoration_box().border = Some(Source::Static(border));

    self
  }

  /// Sets a derived border.
  #[inline(always)]
  #[must_use]
  fn derive_border(mut self, border: impl Fn() -> Border + 'static) -> Self {
    self.decoration_box().border = Some(Source::Derived(Rc::new(border)));

    self
  }

  /// Sets a static border radius.
  #[inline(always)]
  #[must_use]
  fn set_border_radius(mut self, border_radius: f32) -> Self {
    self.decoration_box().border_radius = Some(Source::Static(border_radius));

    self
  }

  /// Sets a derived border radius.
  #[inline(always)]
  #[must_use]
  fn derive_border_radius(mut self, border_radius: impl Fn() -> f32 + 'static) -> Self {
    self.decoration_box().border_radius = Some(Source::Derived(Rc::new(border_radius)));

    self
  }

  /// Sets a static mouse cursor.
  #[inline(always)]
  #[must_use]
  fn set_cursor(mut self, cursor: MouseCursor) -> Self {
    self.decoration_box().cursor = Some(Source::Static(cursor));

    self
  }

  /// Sets a derived mouse cursor.
  #[inline(always)]
  #[must_use]
  fn derive_cursor(mut self, cursor: impl Fn() -> MouseCursor + 'static) -> Self {
    self.decoration_box().cursor = Some(Source::Derived(Rc::new(cursor)));

    self
  }

  /// Sets a static overflow behavior.
  #[inline(always)]
  #[must_use]
  fn set_overflow(mut self, overflow: Overflow) -> Self {
    self.decoration_box().overflow = Some(Source::Static(overflow));

    self
  }

  /// Sets a derived overflow behavior.
  #[inline(always)]
  #[must_use]
  fn derive_overflow(mut self, overflow: impl Fn() -> Overflow + 'static) -> Self {
    self.decoration_box().overflow = Some(Source::Derived(Rc::new(overflow)));

    self
  }

  /// Sets the semantic role.
  #[inline(always)]
  #[must_use]
  fn set_role(mut self, role: Role) -> Self {
    self.decoration_box().role = Some(role);

    self
  }

  /// Adds a conditional style variant, applied while `condition` holds.
  ///
  /// Declarative: it compiles to a stylesheet rule, so it works in static output with
  /// no script. Later variants win over earlier ones.
  #[inline(always)]
  #[must_use]
  fn set_when(mut self, condition: Condition, style: BoxStyle) -> Self {
    self.decoration_box().variants.push((condition, style));

    self
  }

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  #[must_use]
  fn set_transition(mut self, transition: Transition) -> Self {
    self.decoration_box().transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  #[must_use]
  fn set_animation(mut self, animation: Animation<BoxStyle>) -> Self {
    self.decoration_box().animations.push(animation);

    self
  }

  /// Binds `anchor` to the box, so an overlay's
  /// [`set_anchor`](crate::Overlay::set_anchor) positions against it once built.
  #[inline(always)]
  #[must_use]
  fn bind_anchor(mut self, anchor: &crate::Anchor) -> Self {
    self.decoration_box().anchor = Some(anchor.clone());

    self
  }
}

/// The event surface: click, pointer, focus, key, and scroll handlers, forwarded to a
/// wrapped [`StyledBox`].
pub trait HasEvents: Sized {
  /// The box the handlers land on.
  fn events_box(&mut self) -> &mut StyledBox;

  /// Sets the click handler.
  #[inline(always)]
  #[must_use]
  fn handle_click<R: Into<Prevent>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self.events_box().click_handler = Some(Rc::new(move || action().into()));

    self
  }

  /// Sets the pointer-enter handler.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self.events_box().pointer_enter_handler = Some(Rc::new(action));

    self
  }

  /// Sets the pointer-leave handler.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self.events_box().pointer_leave_handler = Some(Rc::new(action));

    self
  }

  /// Sets the focus handler, run when the box or a descendant gains focus.
  ///
  /// A box never takes focus itself—only controls do; this observes focus within it.
  #[inline(always)]
  #[must_use]
  fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.events_box().focus_handler = Some(Rc::new(action));

    self
  }

  /// Sets the blur handler, run when the box or a descendant loses focus.
  #[inline(always)]
  #[must_use]
  fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.events_box().blur_handler = Some(Rc::new(action));

    self
  }

  /// Sets the key-press handler, run for keys pressed on the box or within it.
  #[inline(always)]
  #[must_use]
  fn handle_key_down<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&KeyEvent) -> R + 'static,
  ) -> Self {
    self.events_box().key_down_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-down handler, run for presses on the box or within it.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_down<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.events_box().pointer_down_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-up handler, run for releases on the box or within it.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_up<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.events_box().pointer_up_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-move handler, run for moves over the box or within it.
  #[inline(always)]
  #[must_use]
  fn handle_pointer_move<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.events_box().pointer_move_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the scroll handler, run with the new offsets when the box scrolls.
  ///
  /// The box scrolls only with an overflow that allows it; see
  /// [`set_overflow`](HasDecoration::set_overflow).
  #[inline(always)]
  #[must_use]
  fn handle_scroll(mut self, action: impl Fn(&ScrollEvent) + 'static) -> Self {
    self.events_box().scroll_handler = Some(Rc::new(action));

    self
  }
}

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
