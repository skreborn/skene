//! The surface every form control shares: styling, conditions, dynamics, and focus.

// Fields are read only by a platform's realization; a target with no platform compiles
// the base but never reads them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::rc::Rc;

use skene_core::{Animation, BoxStyle, Condition, KeyEvent, Transition};

use crate::FocusHandle;

/// The state behind a control's shared builder surface, embedded by every control.
pub(crate) struct ControlBase {
  pub(crate) style: BoxStyle,
  pub(crate) variants: Vec<(Condition, BoxStyle)>,
  pub(crate) transitions: Vec<Transition>,
  pub(crate) animations: Vec<Animation<BoxStyle>>,
  pub(crate) tab_index: Option<i32>,
  pub(crate) autofocus: bool,
  pub(crate) focus_handle: Option<FocusHandle>,
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
      transitions: Vec::new(),
      animations: Vec::new(),
      tab_index: None,
      autofocus: false,
      focus_handle: None,
      focus_handler: None,
      blur_handler: None,
      key_down_handler: None,
    }
  }

  /// Splits the base into the realization props and the handlers to register on the
  /// built node.
  #[cfg(target_arch = "wasm32")]
  pub(crate) fn into_parts(self) -> (skene_core::ControlProps, ControlHandlers) {
    (
      skene_core::ControlProps {
        style: self.style,
        variants: self.variants,
        transitions: self.transitions,
        animations: self.animations,
        tab_index: self.tab_index,
        autofocus: self.autofocus,
      },
      ControlHandlers {
        focus_handle: self.focus_handle,
        focus: self.focus_handler,
        blur: self.blur_handler,
        key_down: self.key_down_handler,
      },
    )
  }
}

/// The shared handlers split off a [`ControlBase`], registered once the node exists.
#[cfg(target_arch = "wasm32")]
pub(crate) struct ControlHandlers {
  focus_handle: Option<FocusHandle>,
  focus: Option<Rc<dyn Fn()>>,
  blur: Option<Rc<dyn Fn()>>,
  #[expect(clippy::type_complexity)]
  key_down: Option<Rc<dyn Fn(&KeyEvent)>>,
}

#[cfg(target_arch = "wasm32")]
impl ControlHandlers {
  /// Registers the handlers on `node` and binds the focus handle to it.
  pub(crate) fn register(self, node: skene_core::NodeId) {
    if let Some(action) = self.focus {
      crate::driver::handle_focus(node, action);
    }

    if let Some(action) = self.blur {
      crate::driver::handle_blur(node, action);
    }

    if let Some(action) = self.key_down {
      crate::driver::handle_key_down(node, action);
    }

    if let Some(handle) = &self.focus_handle {
      handle.assign(node);
    }
  }
}

/// Implements the shared control surface on a control with a `control: ControlBase`
/// field: styling, conditional variants, transitions, animations, tab order, autofocus,
/// focus binding, and the focus/blur/key handlers.
macro_rules! control_surface {
  ($control:ident) => {
    impl $control {
      /// Sets the static style.
      #[inline(always)]
      pub fn set_style(mut self, style: skene_core::BoxStyle) -> Self {
        self.control.style = style;

        self
      }

      /// Adds a conditional style variant, applied while `condition` holds.
      #[inline(always)]
      pub fn set_when(
        mut self,
        condition: skene_core::Condition,
        style: skene_core::BoxStyle,
      ) -> Self {
        self.control.variants.push((condition, style));

        self
      }

      /// Adds a property transition, interpolating that property whenever its value
      /// changes.
      #[inline(always)]
      pub fn set_transition(mut self, transition: skene_core::Transition) -> Self {
        self.control.transitions.push(transition);

        self
      }

      /// Adds a keyframe animation.
      #[inline(always)]
      pub fn set_animation(
        mut self,
        animation: skene_core::Animation<skene_core::BoxStyle>,
      ) -> Self {
        self.control.animations.push(animation);

        self
      }

      /// Sets the position in the keyboard tab order: `0` joins it in document order, a
      /// negative value removes the control from it while keeping it programmatically
      /// focusable.
      #[inline(always)]
      pub fn set_tab_index(mut self, tab_index: i32) -> Self {
        self.control.tab_index = Some(tab_index);

        self
      }

      /// Makes the control take focus when the page loads.
      #[inline(always)]
      pub fn set_autofocus(mut self) -> Self {
        self.control.autofocus = true;

        self
      }

      /// Binds `handle` to the control, so
      /// [`FocusHandle::focus`](crate::FocusHandle::focus) targets it once built.
      #[inline(always)]
      pub fn bind_focus(mut self, handle: &crate::FocusHandle) -> Self {
        self.control.focus_handle = Some(handle.clone());

        self
      }

      /// Sets the focus handler.
      #[inline(always)]
      pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
        self.control.focus_handler = Some(std::rc::Rc::new(action));

        self
      }

      /// Sets the blur handler.
      #[inline(always)]
      pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
        self.control.blur_handler = Some(std::rc::Rc::new(action));

        self
      }

      /// Sets the key-press handler.
      #[inline(always)]
      pub fn handle_key_down(mut self, action: impl Fn(&skene_core::KeyEvent) + 'static) -> Self {
        self.control.key_down_handler = Some(std::rc::Rc::new(action));

        self
      }
    }
  };
}

pub(crate) use control_surface;
