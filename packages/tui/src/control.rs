//! The surface every terminal control shares: styling, conditions, and focus.

use std::rc::Rc;

use skene_core::{KeyEvent, NodeId};

use crate::{
  FocusHandle,
  props::{BoxStyle, Condition},
  with_state,
};

/// The state behind a control's shared builder surface, embedded by every control.
pub(crate) struct ControlBase {
  pub(crate) style: BoxStyle,
  pub(crate) variants: Vec<(Condition, BoxStyle)>,
  pub(crate) autofocus: bool,
  pub(crate) focus_handle: Option<FocusHandle>,
  pub(crate) anchor: Option<crate::Anchor>,
  pub(crate) focus_handler: Option<Rc<dyn Fn()>>,
  pub(crate) blur_handler: Option<Rc<dyn Fn()>>,
  #[expect(clippy::type_complexity)]
  pub(crate) key_down_handler: Option<Rc<dyn Fn(&KeyEvent)>>,
}

impl ControlBase {
  pub(crate) fn new() -> Self {
    Self {
      style: BoxStyle::new(),
      variants: Vec::new(),
      autofocus: false,
      focus_handle: None,
      anchor: None,
      focus_handler: None,
      blur_handler: None,
      key_down_handler: None,
    }
  }

  /// Registers the built control: joins the focus order, wires the base's handlers,
  /// and binds its handles.
  pub(crate) fn register(self, node: NodeId) {
    with_state(|state| {
      state.controls.borrow_mut().insert(node);

      if let Some(action) = self.focus_handler {
        state
          .focus
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.blur_handler {
        state
          .blur
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.key_down_handler {
        state
          .key_down
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if self.autofocus {
        state.focused.set(Some(node));
        state.dirty.set(true);
      }
    });

    if let Some(handle) = &self.focus_handle {
      handle.assign(node);
    }

    if let Some(anchor) = &self.anchor {
      anchor.assign(node);
    }
  }
}

/// Generates the shared builder surface, delegating to the embedded [`ControlBase`].
macro_rules! control_surface {
  ($name:ident) => {
    impl $name {
      /// Sets the static style.
      #[inline(always)]
      pub fn set_style(mut self, style: crate::props::BoxStyle) -> Self {
        self.base.style = style;

        self
      }

      /// Adds a conditional style variant, applied while `condition` holds.
      #[inline(always)]
      pub fn set_when(
        mut self,
        condition: crate::props::Condition,
        style: crate::props::BoxStyle,
      ) -> Self {
        self.base.variants.push((condition, style));

        self
      }

      /// Sets the focus handler.
      #[inline(always)]
      pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
        self.base.focus_handler = Some(std::rc::Rc::new(action));

        self
      }

      /// Sets the blur handler.
      #[inline(always)]
      pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
        self.base.blur_handler = Some(std::rc::Rc::new(action));

        self
      }

      /// Sets the key-press handler, run for keys pressed while the control is focused.
      #[inline(always)]
      pub fn handle_key_down(mut self, action: impl Fn(&skene_core::KeyEvent) + 'static) -> Self {
        self.base.key_down_handler = Some(std::rc::Rc::new(action));

        self
      }

      /// Makes the control take focus when it mounts.
      #[inline(always)]
      pub fn set_autofocus(mut self) -> Self {
        self.base.autofocus = true;

        self
      }

      /// Binds `handle` to the control, so
      /// [`FocusHandle::focus`](crate::FocusHandle::focus) targets it once built.
      #[inline(always)]
      pub fn bind_focus(mut self, handle: &crate::FocusHandle) -> Self {
        self.base.focus_handle = Some(handle.clone());

        self
      }

      /// Binds `anchor` to the control, so an overlay's
      /// [`set_anchor`](crate::Overlay::set_anchor) positions against it once built.
      #[inline(always)]
      pub fn bind_anchor(mut self, anchor: &crate::Anchor) -> Self {
        self.base.anchor = Some(anchor.clone());

        self
      }
    }
  };
}

pub(crate) use control_surface;
