//! The surface every form control shares: styling, conditions, dynamics, and focus.

// Fields are read only by a platform's realization; a target with no platform compiles
// the base but never reads them.

use std::rc::Rc;

use skene_core::{Animation, BoxStyle, Condition, Transition};

use crate::FocusHandle;

/// The state behind a control's shared builder surface, embedded by every control.
pub(crate) struct ControlBase {
  pub(crate) style: BoxStyle,
  pub(crate) variants: Vec<(Condition, BoxStyle)>,
  pub(crate) transitions: Vec<Transition>,
  pub(crate) animations: Vec<Animation<BoxStyle>>,
  pub(crate) focus_order: Option<skene_core::FocusOrder>,
  pub(crate) autofocus: bool,
  pub(crate) focus_handle: Option<FocusHandle>,
  pub(crate) anchor: Option<crate::Anchor>,
  pub(crate) focus_handler: Option<Rc<dyn Fn()>>,
  pub(crate) blur_handler: Option<Rc<dyn Fn()>>,
  pub(crate) key_down_handler: Option<Rc<skene_core::KeyHandler>>,
}

impl ControlBase {
  pub(crate) fn new() -> Self {
    Self {
      style: BoxStyle::new(),
      variants: Vec::new(),
      transitions: Vec::new(),
      animations: Vec::new(),
      focus_order: None,
      autofocus: false,
      focus_handle: None,
      anchor: None,
      focus_handler: None,
      blur_handler: None,
      key_down_handler: None,
    }
  }

  /// Splits the base into the realization props and the handlers to register on the
  /// built node, resolving the inherited text style into the props' static text and
  /// the handlers' reactive binders.
  pub(crate) fn into_parts(self) -> (skene_core::ControlProps, ControlHandlers) {
    use skene_core::{RunProp, RunStyle};

    use crate::realize::split_prop;

    let resolved = crate::text::resolved_text_style(crate::TextStyle::new(), false);
    let mut text = RunStyle::default();
    let mut binders = crate::realize::Binders::new();

    text.selection_color = resolved.selection_color;
    text.selection_background = resolved.selection_background;

    split_prop(
      resolved.color,
      &mut text.color,
      &mut binders,
      RunProp::Color,
      crate::driver::update_run,
    );

    split_prop(
      resolved.font_size,
      &mut text.font_size,
      &mut binders,
      RunProp::FontSize,
      crate::driver::update_run,
    );

    split_prop(
      resolved.font_weight,
      &mut text.font_weight,
      &mut binders,
      RunProp::FontWeight,
      crate::driver::update_run,
    );

    (
      skene_core::ControlProps {
        style: self.style,
        text,
        variants: self.variants,
        transitions: self.transitions,
        animations: self.animations,
        focus_order: self.focus_order,
        autofocus: self.autofocus,
      },
      ControlHandlers {
        focus_handle: self.focus_handle,
        anchor: self.anchor,
        focus: self.focus_handler,
        blur: self.blur_handler,
        key_down: self.key_down_handler,
        binders,
      },
    )
  }
}

/// The shared handlers split off a [`ControlBase`], registered once the node exists.
pub(crate) struct ControlHandlers {
  focus_handle: Option<FocusHandle>,
  anchor: Option<crate::Anchor>,
  focus: Option<Rc<dyn Fn()>>,
  blur: Option<Rc<dyn Fn()>>,
  key_down: Option<Rc<skene_core::KeyHandler>>,
  binders: crate::realize::Binders,
}

impl ControlHandlers {
  /// Registers the handlers on `node`, runs the reactive text-style binders, and binds
  /// the focus handle to it.
  pub(crate) fn register(self, node: skene_core::NodeId) {
    for binder in self.binders {
      binder(node);
    }

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

    if let Some(anchor) = &self.anchor {
      anchor.assign(node);
    }
  }
}

/// Wraps a built toggle control in a label box carrying its caption, so activating the
/// caption activates the control; returns the wrapper.
pub(crate) fn labeled(control: skene_core::NodeId, label: String) -> skene_core::NodeId {
  use skene_core::{BoxProps, BoxStyle, FlexDirection, ItemAlignment, Layout, View};

  let wrapper = crate::driver::create_box(&BoxProps {
    layout: Layout::Flex {
      direction: FlexDirection::Row,
    },
    role: Some(skene_core::Role::Label),
    style: BoxStyle::new()
      .set_spacing(8.)
      .set_item_alignment(ItemAlignment::Center),
    variants: Vec::new(),
    transitions: Vec::new(),
    animations: Vec::new(),
  });

  crate::driver::attach(wrapper, control, None);

  let caption = crate::Text::new(label).build();

  crate::driver::attach(wrapper, caption, None);

  wrapper
}

/// Implements the shared control surface on a control with a `control: ControlBase`
/// field: styling, conditional variants, transitions, animations, focus order,
/// autofocus, focus binding, and the focus/blur/key handlers.
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

      /// Sets the control's place in sequential focus traversal.
      #[inline(always)]
      pub fn set_focus_order(mut self, focus_order: skene_core::FocusOrder) -> Self {
        self.control.focus_order = Some(focus_order);

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

      /// Binds `anchor` to the control, so an overlay's
      /// [`set_anchor`](crate::Overlay::set_anchor) positions against it once built.
      #[inline(always)]
      pub fn bind_anchor(mut self, anchor: &crate::Anchor) -> Self {
        self.control.anchor = Some(anchor.clone());

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
      pub fn handle_key_down<R: Into<skene_core::Prevent>>(
        mut self,
        action: impl Fn(&skene_core::KeyEvent) -> R + 'static,
      ) -> Self {
        self.control.key_down_handler = Some(std::rc::Rc::new(move |event| action(event).into()));

        self
      }
    }
  };
}

pub(crate) use control_surface;
